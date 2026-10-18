use super::*;
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

#[test]
fn reset_mail_uses_the_existing_smtp_sender_with_the_link_and_expiry() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(stream) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "SMTP fixture connection timeout");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("SMTP fixture failed: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        stream.write_all(b"220 localhost test SMTP\r\n").unwrap();
        loop {
            let mut line = String::new();
            assert_ne!(reader.read_line(&mut line).unwrap(), 0);
            if line.starts_with("EHLO") || line.starts_with("HELO") {
                stream.write_all(b"250 localhost\r\n").unwrap();
            } else if line.starts_with("MAIL FROM:") || line.starts_with("RCPT TO:") {
                stream.write_all(b"250 OK\r\n").unwrap();
            } else if line.trim_end() == "DATA" {
                stream.write_all(b"354 Send message\r\n").unwrap();
                let mut message = String::new();
                loop {
                    line.clear();
                    assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                    if line == ".\r\n" {
                        break;
                    }
                    message.push_str(&line);
                }
                stream.write_all(b"250 Message accepted\r\n").unwrap();
                return message;
            } else {
                panic!("Unexpected SMTP fixture command");
            }
        }
    });
    let mailer = SmtpMailer {
        transport: SmtpTransport::builder_dangerous("127.0.0.1")
            .port(port)
            .timeout(Some(Duration::from_secs(5)))
            .build(),
        from: "sender@example.com".parse().unwrap(),
    };
    let link = "https://agent.example/reset-password#token=test-link";
    let message = RegistrationEmailMessage::new(
        "Password reset".into(),
        format!("{link}\nValid for 20 minutes; one use."),
    );
    let sent = mailer.send_password_reset("person@example.com", link, &message);
    let received = worker
        .join()
        .unwrap()
        .replace("=\r\n", "")
        .replace("=3D", "=");
    assert!(sent.is_ok());
    assert!(received.contains("To: person@example.com"));
    assert!(received.contains("Subject: Password reset"));
    assert!(received.contains(link));
    assert!(received.contains("20 minutes"));
}
