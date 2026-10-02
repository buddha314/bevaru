//! MNIST, downloaded on first use, verified against pinned SHA-256 checksums,
//! and cached. Set `BEVARU_MNIST_DIR` to use a pre-downloaded copy (the four
//! original `*-ubyte.gz` files) instead of the user cache directory.

use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use nalgebra::DMatrix;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;
use sha2::{Digest, Sha256};

use crate::dataset::{Dataset, Targets};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MnistFile {
    pub name: &'static str,
    pub sha256: &'static str,
}

/// Checksums of the canonical files; their MD5s match those published by
/// torchvision.
pub const FILES: [MnistFile; 4] = [
    MnistFile {
        name: "train-images-idx3-ubyte.gz",
        sha256: "440fcabf73cc546fa21475e81ea370265605f56be210a4024d2ca8f203523609",
    },
    MnistFile {
        name: "train-labels-idx1-ubyte.gz",
        sha256: "3552534a0a558bbed6aed32b30c495cca23d567ec52cac8be1a0730e8010255c",
    },
    MnistFile {
        name: "t10k-images-idx3-ubyte.gz",
        sha256: "8d422c7b0a1c1c79245a5bcf07fe86e33eeafee792b84584aec276f5a2dbc4e6",
    },
    MnistFile {
        name: "t10k-labels-idx1-ubyte.gz",
        sha256: "f7ae60f92e00ec6debd23a6088c31dbd2371eca3ffa0defaefb259924204aec6",
    },
];

pub const MIRRORS: [&str; 2] = [
    "https://ossci-datasets.s3.amazonaws.com/mnist/",
    "https://storage.googleapis.com/cvdf-datasets/mnist/",
];

pub const IMAGE_SIDE: usize = 28;
pub const PIXELS: usize = IMAGE_SIDE * IMAGE_SIDE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Split {
    Train,
    Test,
}

/// Which samples to load. Filtering and subsampling happen before pixels are
/// converted, so a small subset never materializes the full 60 000 images.
#[derive(Debug, Clone, PartialEq)]
pub struct MnistOptions {
    pub split: Split,
    /// Keep only these digits; `None` keeps all ten.
    pub digits: Option<Vec<u8>>,
    pub max_samples: Option<usize>,
    pub seed: u64,
}

impl Default for MnistOptions {
    fn default() -> Self {
        Self {
            split: Split::Train,
            digits: None,
            max_samples: Some(2000),
            seed: 0,
        }
    }
}

#[derive(Debug)]
pub enum MnistError {
    Checksum {
        file: String,
        expected: String,
        actual: String,
    },
    Download {
        file: String,
        error: String,
    },
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
    Format {
        file: String,
        error: String,
    },
    NoCacheDir,
}

impl fmt::Display for MnistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MnistError::Checksum {
                file,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "{file}: checksum mismatch (expected sha256 {expected}, got {actual})"
                )
            }
            MnistError::Download { file, error } => write!(f, "{file}: download failed: {error}"),
            MnistError::Io { path, error } => write!(f, "{}: {error}", path.display()),
            MnistError::Format { file, error } => write!(f, "{file}: {error}"),
            MnistError::NoCacheDir => {
                f.write_str("no cache directory available; set BEVARU_MNIST_DIR")
            }
        }
    }
}

impl std::error::Error for MnistError {}

type Fetch = Box<dyn Fn(&str) -> Result<Vec<u8>, String> + Send + Sync>;

pub struct MnistLoader {
    cache_dir: PathBuf,
    mirrors: Vec<String>,
    files: [MnistFile; 4],
    fetch: Fetch,
}

impl MnistLoader {
    /// Cache in `BEVARU_MNIST_DIR`, else `<user cache>/bevaru/mnist`; download
    /// with `ureq`.
    pub fn new() -> Result<Self, MnistError> {
        let cache_dir = match std::env::var_os("BEVARU_MNIST_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => dirs::cache_dir()
                .ok_or(MnistError::NoCacheDir)?
                .join("bevaru")
                .join("mnist"),
        };
        Ok(Self::with_cache_dir(cache_dir))
    }

    pub fn with_cache_dir(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            mirrors: MIRRORS.iter().map(|s| s.to_string()).collect(),
            files: FILES,
            fetch: Box::new(http_get),
        }
    }

    /// Replace the downloader (for tests or custom transports).
    pub fn with_fetcher(
        mut self,
        fetch: impl Fn(&str) -> Result<Vec<u8>, String> + Send + Sync + 'static,
    ) -> Self {
        self.fetch = Box::new(fetch);
        self
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn load(&self, opts: &MnistOptions) -> Result<Dataset, MnistError> {
        let (images, labels) = match opts.split {
            Split::Train => (self.files[0], self.files[1]),
            Split::Test => (self.files[2], self.files[3]),
        };
        let labels_raw = gunzip(labels.name, &self.file_bytes(&labels)?)?;
        let labels_all = parse_idx(labels.name, &labels_raw, 2049, &[])?;
        let images_raw = gunzip(images.name, &self.file_bytes(&images)?)?;
        let pixels = parse_idx(images.name, &images_raw, 2051, &[IMAGE_SIDE, IMAGE_SIDE])?;
        if pixels.len() != labels_all.len() * PIXELS {
            return Err(MnistError::Format {
                file: images.name.into(),
                error: format!(
                    "{} images for {} labels",
                    pixels.len() / PIXELS,
                    labels_all.len()
                ),
            });
        }

        let mut idx: Vec<usize> = (0..labels_all.len())
            .filter(|&i| {
                opts.digits
                    .as_ref()
                    .is_none_or(|d| d.contains(&labels_all[i]))
            })
            .collect();
        if let Some(n) = opts.max_samples.filter(|&n| n < idx.len()) {
            idx.shuffle(&mut ChaCha8Rng::seed_from_u64(opts.seed));
            idx.truncate(n);
            idx.sort_unstable();
        }

        let mut features = DMatrix::zeros(idx.len(), PIXELS);
        for (r, &i) in idx.iter().enumerate() {
            for (c, &p) in pixels[i * PIXELS..(i + 1) * PIXELS].iter().enumerate() {
                features[(r, c)] = p as f64 / 255.0;
            }
        }
        Ok(Dataset {
            name: format!(
                "MNIST ({})",
                if opts.split == Split::Train {
                    "train"
                } else {
                    "test"
                }
            ),
            features,
            feature_names: (0..PIXELS)
                .map(|i| format!("pixel ({}, {})", i / IMAGE_SIDE, i % IMAGE_SIDE))
                .collect(),
            targets: Targets::Classes {
                labels: idx.iter().map(|&i| labels_all[i] as usize).collect(),
                names: (0..10).map(|d| d.to_string()).collect(),
            },
        })
    }

    /// Verified bytes of one file, from the cache or downloaded into it.
    fn file_bytes(&self, file: &MnistFile) -> Result<Vec<u8>, MnistError> {
        let path = self.cache_dir.join(file.name);
        if let Ok(bytes) = fs::read(&path)
            && sha256_hex(&bytes) == file.sha256
        {
            return Ok(bytes);
        }
        // A corrupt cache entry is replaced, never trusted.
        let mut last_err = None;
        for mirror in &self.mirrors {
            let url = format!("{mirror}{}", file.name);
            let bytes = match (self.fetch)(&url) {
                Ok(b) => b,
                Err(error) => {
                    last_err = Some(MnistError::Download {
                        file: file.name.into(),
                        error,
                    });
                    continue;
                }
            };
            let actual = sha256_hex(&bytes);
            if actual != file.sha256 {
                last_err = Some(MnistError::Checksum {
                    file: file.name.into(),
                    expected: file.sha256.into(),
                    actual,
                });
                continue;
            }
            self.store(&path, &bytes)?;
            return Ok(bytes);
        }
        Err(last_err.unwrap_or(MnistError::Download {
            file: file.name.into(),
            error: "no mirrors".into(),
        }))
    }

    /// Write via a temporary file so a crash never leaves a partial entry.
    fn store(&self, path: &Path, bytes: &[u8]) -> Result<(), MnistError> {
        let io = |error| MnistError::Io {
            path: path.to_path_buf(),
            error,
        };
        fs::create_dir_all(&self.cache_dir).map_err(io)?;
        let tmp = path.with_extension("partial");
        fs::write(&tmp, bytes).map_err(io)?;
        fs::rename(&tmp, path).map_err(io)
    }
}

fn http_get(url: &str) -> Result<Vec<u8>, String> {
    let mut resp = ureq::get(url).call().map_err(|e| e.to_string())?;
    resp.body_mut()
        .with_config()
        .limit(64 << 20)
        .read_to_vec()
        .map_err(|e| e.to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn gunzip(name: &str, bytes: &[u8]) -> Result<Vec<u8>, MnistError> {
    let mut out = Vec::new();
    GzDecoder::new(bytes)
        .read_to_end(&mut out)
        .map_err(|e| MnistError::Format {
            file: name.into(),
            error: e.to_string(),
        })?;
    Ok(out)
}

/// Parse an IDX file with the given magic number and trailing dimensions;
/// returns the payload.
fn parse_idx<'a>(
    name: &str,
    raw: &'a [u8],
    magic: u32,
    dims: &[usize],
) -> Result<&'a [u8], MnistError> {
    let err = |e: String| MnistError::Format {
        file: name.into(),
        error: e,
    };
    let word = |i: usize| -> Result<u32, MnistError> {
        raw.get(4 * i..4 * i + 4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
            .ok_or_else(|| err("truncated header".into()))
    };
    let m = word(0)?;
    if m != magic {
        return Err(err(format!("bad magic {m:#x}, expected {magic:#x}")));
    }
    let n = word(1)? as usize;
    for (k, &d) in dims.iter().enumerate() {
        let got = word(2 + k)? as usize;
        if got != d {
            return Err(err(format!("dimension {k} is {got}, expected {d}")));
        }
    }
    let header = 4 * (2 + dims.len());
    let len = n * dims.iter().product::<usize>();
    raw.get(header..header + len)
        .ok_or_else(|| err(format!("payload shorter than {len} bytes")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    fn gz(bytes: &[u8]) -> Vec<u8> {
        let mut e = GzEncoder::new(Vec::new(), Compression::fast());
        e.write_all(bytes).unwrap();
        e.finish().unwrap()
    }

    /// A tiny valid MNIST: `n` images where image i is filled with value i
    /// and labelled `i % 10`.
    fn fake_files(n: usize) -> Vec<(&'static str, Vec<u8>)> {
        let mut images = [2051u32, n as u32, 28, 28]
            .iter()
            .flat_map(|w| w.to_be_bytes())
            .collect::<Vec<_>>();
        for i in 0..n {
            images.extend(std::iter::repeat_n(i as u8, PIXELS));
        }
        let mut labels = [2049u32, n as u32]
            .iter()
            .flat_map(|w| w.to_be_bytes())
            .collect::<Vec<_>>();
        labels.extend((0..n).map(|i| (i % 10) as u8));
        let (images, labels) = (gz(&images), gz(&labels));
        vec![
            (FILES[0].name, images.clone()),
            (FILES[1].name, labels.clone()),
            (FILES[2].name, images),
            (FILES[3].name, labels),
        ]
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bevaru-mnist-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// A loader whose pinned checksums match `files`, serving them from memory
    /// and counting fetches.
    fn fake_loader(
        dir: &Path,
        files: Vec<(&'static str, Vec<u8>)>,
        calls: Arc<AtomicUsize>,
    ) -> MnistLoader {
        let mut loader = MnistLoader::with_cache_dir(dir);
        let checksums: Vec<&'static str> = files
            .iter()
            .map(|(_, b)| &*Box::leak(sha256_hex(b).into_boxed_str()))
            .collect();
        for (f, sum) in loader.files.iter_mut().zip(checksums) {
            f.sha256 = sum;
        }
        loader.with_fetcher(move |url| {
            calls.fetch_add(1, Ordering::SeqCst);
            let name = url.rsplit('/').next().unwrap();
            Ok(files.iter().find(|(n, _)| *n == name).unwrap().1.clone())
        })
    }

    #[test]
    fn checksum_mismatch_is_rejected_and_not_cached() {
        let dir = temp_dir("mismatch");
        let loader = MnistLoader::with_cache_dir(&dir).with_fetcher(|_| Ok(b"not mnist".to_vec()));
        let err = loader.load(&MnistOptions::default()).unwrap_err();
        let MnistError::Checksum { file, .. } = &err else {
            panic!("{err}")
        };
        assert_eq!(file, "train-labels-idx1-ubyte.gz");
        assert!(err.to_string().contains("train-labels-idx1-ubyte.gz"));
        assert!(!dir.join(file).exists());
    }

    #[test]
    fn cached_reload_makes_no_network_call() {
        let dir = temp_dir("cache");
        let calls = Arc::new(AtomicUsize::new(0));
        let loader = fake_loader(&dir, fake_files(30), calls.clone());
        let opts = MnistOptions {
            max_samples: None,
            ..Default::default()
        };
        let first = loader.load(&opts).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let second = loader.load(&opts).unwrap();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "second load must hit the cache only"
        );
        assert_eq!(first, second);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn digit_filter_and_subsample() {
        let dir = temp_dir("filter");
        let loader = fake_loader(&dir, fake_files(100), Arc::new(AtomicUsize::new(0)));
        let opts = MnistOptions {
            digits: Some(vec![3, 8]),
            max_samples: Some(12),
            seed: 4,
            ..Default::default()
        };
        let d = loader.load(&opts).unwrap();
        assert_eq!(d.len(), 12);
        assert_eq!(d.dim(), PIXELS);
        let Targets::Classes { labels, .. } = &d.targets else {
            panic!()
        };
        assert!(labels.iter().all(|&l| l == 3 || l == 8));
        // Pixel values carry the image index, which must agree with the label.
        for (r, &l) in labels.iter().enumerate() {
            let i = (d.features[(r, 0)] * 255.0).round() as usize;
            assert_eq!(i % 10, l);
        }
        assert_eq!(d, loader.load(&opts).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_cache_entry_is_replaced() {
        let dir = temp_dir("corrupt");
        let calls = Arc::new(AtomicUsize::new(0));
        let loader = fake_loader(&dir, fake_files(10), calls.clone());
        let opts = MnistOptions {
            max_samples: None,
            ..Default::default()
        };
        loader.load(&opts).unwrap();
        fs::write(dir.join(FILES[1].name), b"garbage").unwrap();
        loader.load(&opts).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        let _ = fs::remove_dir_all(&dir);
    }
}
