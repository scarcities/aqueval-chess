use std::io::Write;

/// Engine configurable options adhering to UCI specification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UciOptions {
    pub hash_mb: usize,
    pub threads: usize,
    pub move_overhead_ms: u64,
}

impl Default for UciOptions {
    fn default() -> Self {
        Self {
            hash_mb: 16,
            threads: 1,
            move_overhead_ms: 30,
        }
    }
}

impl UciOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures an option by name and optional value.
    /// Name comparison is strictly case-insensitive.
    pub fn set_option(&mut self, name: &str, value: Option<&str>) {
        let name_lower = name.trim().to_ascii_lowercase();
        match name_lower.as_str() {
            "hash" => {
                if let Some(val_str) = value {
                    if let Ok(val) = val_str.trim().parse::<usize>() {
                        self.hash_mb = val.clamp(1, 32768);
                    }
                }
            }
            "threads" => {
                if let Some(val_str) = value {
                    if let Ok(val) = val_str.trim().parse::<usize>() {
                        self.threads = val.clamp(1, 1);
                    }
                }
            }
            "move overhead" => {
                if let Some(val) = value.and_then(|v| v.trim().parse::<u64>().ok()) {
                    self.move_overhead_ms = val.min(5000);
                }
            }
            _ => {
                // Ignore unknown options gracefully per UCI specification
            }
        }
    }

    /// Prints available options during the `uci` handshake.
    pub fn print_options<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writeln!(writer, "option name Hash type spin default 16 min 1 max 1024")?;
        writeln!(writer, "option name Threads type spin default 1 min 1 max 1")?;
        writeln!(writer, "option name Move Overhead type spin default 30 min 0 max 5000")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_options() {
        let opts = UciOptions::default();
        assert_eq!(opts.hash_mb, 16);
        assert_eq!(opts.threads, 1);
    }

    #[test]
    fn test_set_option_case_insensitive() {
        let mut opts = UciOptions::default();
        opts.set_option("HASH", Some("64"));
        assert_eq!(opts.hash_mb, 64);

        opts.set_option("hAsH", Some("128"));
        assert_eq!(opts.hash_mb, 128);

        opts.set_option("THREADS", Some("1"));
        assert_eq!(opts.threads, 1);

        // Unknown option should not change anything or panic
        opts.set_option("UnknownOption", Some("foo"));
        assert_eq!(opts.hash_mb, 128);

        opts.set_option("move overhead", Some("100"));
        assert_eq!(opts.move_overhead_ms, 100);
    }
}
