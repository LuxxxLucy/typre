use std::fs;
use std::process::Command;

#[test]
fn exports_documents_and_relative_images() {
    let directory = tempfile::tempdir().unwrap();
    let document = directory.path().join("slides.md");
    image::RgbImage::new(9, 18)
        .save(directory.path().join("image.png"))
        .unwrap();
    fs::write(
        &document,
        "## Content\n\nBefore **bold** and [reference](https://example.com).\n\n\
         - List entry\n\n> Quoted text\n\n中文 e\u{301} 👩‍💻 لا\n\n\
         `◊tree{literal}`\n\n~~~\n◊details[code]{literal}\n~~~\n\n\
         ## Commands\n\nBefore ◊tree{branch} after\n\n\
         ◊figure{\n│  ◊tree{nested}\n│  trailing\n}\n\n\
         | Item | Value |\n| --- | --- |\n| alpha | beta |\n\n\
         ## Image\n\n![Local image](image.png)\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_typre"))
        .args(["--export", document.to_str().unwrap()])
        .current_dir(directory.path().parent().unwrap())
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let text = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "CONTENT",
        "Before",
        "bold",
        "reference",
        "List entry",
        "Quoted",
        "text",
        "中文",
        "e\u{301}",
        "👩‍💻",
        "لا",
        "◊tree{literal}",
        "◊details[code]{literal}",
        "branch",
        "after",
        "nested",
        "trailing",
        "alpha",
        "beta",
        "3 / 3",
        "\x1b]8;;https://example.com",
        "\x1b_Gf=100",
        "\u{10EEEE}",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in {text:?}");
    }
    let destination = directory.path().join("export.bin");
    let saved = Command::new(env!("CARGO_BIN_EXE_typre"))
        .args([
            "--export",
            document.to_str().unwrap(),
            "-o",
            destination.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(saved.status.success(), "{:?}", saved);
    assert!(saved.stdout.is_empty());
    assert_eq!(fs::read(destination).unwrap(), output.stdout);
}

#[test]
fn reports_unreadable_documents() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.md");
    let output = Command::new(env!("CARGO_BIN_EXE_typre"))
        .arg("--export")
        .arg(&missing)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains(missing.to_str().unwrap()));
}

#[cfg(unix)]
mod interactive {
    use super::*;
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::process::CommandExt;
    use std::process::Child;
    use std::sync::mpsc::{self, Receiver};
    use std::time::{Duration, Instant};

    struct Terminal {
        child: Child,
        input: File,
        output: Receiver<Vec<u8>>,
        pending: Vec<u8>,
    }

    impl Terminal {
        fn start(document: &std::path::Path) -> Self {
            let mut master = -1;
            let mut slave = -1;
            let mut size = libc::winsize {
                ws_row: 24,
                ws_col: 80,
                ws_xpixel: 720,
                ws_ypixel: 432,
            };
            assert_eq!(
                unsafe {
                    libc::openpty(
                        &mut master,
                        &mut slave,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        &mut size,
                    )
                },
                0
            );
            let input = unsafe { File::from_raw_fd(master) };
            let slave = unsafe { File::from_raw_fd(slave) };
            let mut command = Command::new(env!("CARGO_BIN_EXE_typre"));
            command
                .arg(document)
                .env("TERM", "xterm-256color")
                .stdin(slave.try_clone().unwrap())
                .stdout(slave.try_clone().unwrap())
                .stderr(slave.try_clone().unwrap());
            let master_fd = input.as_raw_fd();
            unsafe {
                command.pre_exec(move || {
                    libc::close(master_fd);
                    if libc::setsid() == -1 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let child = command.spawn().unwrap();
            let mut reader = input.try_clone().unwrap();
            let (send, output) = mpsc::channel();
            std::thread::spawn(move || {
                let mut buffer = [0; 8192];
                while let Ok(count) = reader.read(&mut buffer) {
                    if count == 0 || send.send(buffer[..count].to_vec()).is_err() {
                        break;
                    }
                }
            });
            Self {
                child,
                input,
                output,
                pending: Vec::new(),
            }
        }

        fn expect(&mut self, text: &str) {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(offset) = self
                    .pending
                    .windows(text.len())
                    .position(|part| part == text.as_bytes())
                {
                    self.pending.drain(..offset + text.len());
                    return;
                }
                let bytes = self
                    .output
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .unwrap_or_else(|error| {
                        panic!(
                            "waiting for {text:?}: {error}; output: {:?}",
                            String::from_utf8_lossy(&self.pending)
                        )
                    });
                self.pending.extend(bytes);
            }
        }

        fn send(&mut self, keys: &str) {
            self.input.write_all(keys.as_bytes()).unwrap();
        }
    }

    impl Drop for Terminal {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    #[test]
    fn presents_navigates_reloads_and_restores_the_terminal() {
        let directory = tempfile::tempdir().unwrap();
        let document = directory.path().join("slides.md");
        fs::write(
            &document,
            "## First\n\nStart here.\n\n## Second\n\nMiddle page.\n\n## Third\n\nEnd here.\n",
        )
        .unwrap();
        let mut terminal = Terminal::start(&document);
        terminal.expect("1 / 3");
        for (keys, page) in [
            ("j", "2 / 3"),
            ("G", "3 / 3"),
            ("k", "2 / 3"),
            ("g", "1 / 3"),
            ("3G", "3 / 3"),
        ] {
            terminal.send(keys);
            terminal.expect(page);
        }
        terminal.send("?");
        terminal.expect("Keys");
        terminal.send("\x1b");
        terminal.expect("3 / 3");
        let replacement = directory.path().join("replacement.md");
        fs::write(&replacement, "## Updated\n\nReplacement content.\n").unwrap();
        fs::rename(replacement, &document).unwrap();
        terminal.expect("Replacement content.");
        terminal.expect("1 / 1");
        terminal.send("q");
        terminal.expect("\x1b[?25h");
        terminal.expect("\x1b[?1049l");
        assert!(terminal.child.wait().unwrap().success());
    }
}
