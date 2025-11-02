#![cfg_attr(not(any(feature = "std", test)), no_std)]

use embassy_futures::join::join;

/// Copies from `read` to `write` in parallel
/// The first chunk is read sequentially
/// and then written to `write` while the next
/// chunk is read concurrently and so furth
/// until `read` returns 0
pub async fn copy_double_buffered<'a, E: Sized>(
    mut read: impl AsyncFnMut(&mut [u8]) -> Result<usize, E>,
    mut write: impl AsyncFnMut(&[u8]) -> Result<(), E>,
    mut buf_a: &'a mut [u8],
    mut buf_b: &'a mut [u8],
) -> Result<(), E> {
    let mut read_a: usize = read(buf_a).await?;
    let mut read_b = 0usize;
    loop {
        match (&mut read_a, &mut read_b) {
            (read_a, 0) if *read_a > 0 => {
                let res = join(read(&mut buf_b), write(&buf_a[..*read_a])).await;
                *read_a = 0;
                match res {
                    (Ok(read), res) => {
                        read_b = read;
                        res?;
                    }
                    (res, _) => {
                        res?;
                    }
                }
            }
            (0, read_b) if *read_b > 0 => {
                let res = join(read(&mut buf_a), write(&buf_b[..*read_b])).await;
                *read_b = 0;
                match res {
                    (Ok(read), res) => {
                        read_a = read;
                        res?;
                    }
                    (res, _) => {
                        res?;
                    }
                }
            }
            (0, 0) => {
                break Ok(());
            }
            (read_a, read_b) => {
                write(&buf_a[..*read_a]).await?;
                write(&buf_b[..*read_b]).await?;
            }
        }
    }
}

#[cfg(feature = "embedded-io-async")]
pub mod eia {

    /// Copies from `src` to `dst` in parallel
    /// The first chunk is read sequentially
    /// and then written to `write` while the next
    /// chunk is read concurrently
    /// ```rust
    /// use copy_double_buffered::eia::copy_double_buffered;
    /// # embassy_futures::block_on(async {
    /// let mut src = [0u8; 1024 * 4];
    /// // Generate some data
    /// src.iter_mut()
    ///     .enumerate()
    ///     .for_each(|(i, v)| *v = (i % 255) as u8);
    /// let mut dst: Vec<u8> = Vec::new();
    /// let [mut buf_a, mut buf_b] = [[0u8; 16]; 2];
    /// copy_double_buffered(&src[..], &mut dst, &mut buf_a[..], &mut buf_b[..])
    ///     .await
    ///     .unwrap();
    /// assert_eq!(&src[..], &dst[..]);
    /// # });
    /// ```
    pub async fn copy_double_buffered<'a, R, W, E>(
        mut src: R,
        mut dst: W,
        buf_a: &'a mut [u8],
        buf_b: &'a mut [u8],
    ) -> Result<(), E>
    where
        R: embedded_io_async::Read<Error = E>,
        W: embedded_io_async::Write<Error = E>,
    {
        crate::copy_double_buffered(
            async move |buf| src.read(buf).await,
            async move |buf| dst.write_all(buf).await,
            buf_a,
            buf_b,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;
    use std::time::Instant;

    use tokio::time::sleep;

    #[tokio::test]
    async fn copy_delayed() {
        let mut src = [0u8; 1024 * 4];
        src.iter_mut()
            .enumerate()
            .for_each(|(i, v)| *v = (i % 255) as u8);
        let mut dst: Vec<u8> = Vec::new();
        let [mut buf_a, mut buf_b] = [[0u8; 64]; 2];
        const DELAY: u64 = 100;
        let begin = Instant::now();
        crate::copy_double_buffered(
            {
                let mut src = &src[..];
                async move |buf| {
                    let read = core::cmp::min(buf.len(), src.len());
                    buf[..read].copy_from_slice(&src[..read]);
                    sleep(Duration::from_millis(DELAY)).await;
                    src = &src[read..];
                    Ok::<usize, ()>(read)
                }
            },
            async |buf| {
                dst.extend_from_slice(buf);
                sleep(Duration::from_millis(DELAY)).await;
                Ok::<(), ()>(())
            },
            &mut buf_a[..],
            &mut buf_b[..],
        )
        .await
        .unwrap();
        assert_eq!(&src[..], &dst[..]);
        dst.clear();
        let duration = Instant::now() - begin;
        let naive_begin = Instant::now();
        let mut buf = [0u8; 16];
        {
            let mut src = &src[..];
            loop {
                let read = core::cmp::min(buf.len(), src.len());
                if read == 0 {
                    break;
                }
                buf[..read].copy_from_slice(&src[..read]);
                sleep(Duration::from_millis(DELAY)).await;
                src = &src[read..];
                dst.extend_from_slice(&buf[..]);
                sleep(Duration::from_millis(DELAY)).await;
            }
        }
        let native_duration = Instant::now() - naive_begin;
        dbg!((duration, native_duration));
        assert!(duration * 2 < native_duration * 3);
    }
}
