"""Write a signed RFC 822 email to stdout for submission by your mail client."""

import argparse
import base64
from email.message import EmailMessage
from email.policy import SMTP
import hmac
from pathlib import Path
import sys
import time
import unicodedata
from uuid import uuid4


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--from", dest="sender", required=True)
    parser.add_argument("--to", dest="mailbox", required=True)
    parser.add_argument("--subject", required=True)
    parser.add_argument("--key-file", type=Path, required=True)
    args = parser.parse_args()
    subject = args.subject.strip()
    body = sys.stdin.read(12_001).replace("\r\n", "\n").rstrip("\n")
    if not 0 < len(subject) <= 160 or not 0 < len(body) <= 12_000:
        parser.error("The subject and body must fit the mail task limits")
    if any(unicodedata.category(character) == "Cc" and character not in "\n\t"
           for character in subject + body) or "\n" in subject or "\t" in subject:
        parser.error("The subject and body must contain plain text")
    sender_local, sender_domain = args.sender.rsplit("@", 1)
    mailbox_local, mailbox_domain = args.mailbox.rsplit("@", 1)
    sender, mailbox = f"{sender_local}@{sender_domain.lower()}", f"{mailbox_local}@{mailbox_domain.lower()}"
    key = base64.b64decode(args.key_file.read_text().strip(), validate=True)
    if len(key) != 32:
        parser.error("Expected a base64 encoded, 32-byte signing key")
    timestamp, nonce = str(int(time.time())), str(uuid4())
    fields = [mailbox, sender, timestamp, nonce, subject, body]
    payload = b"gosling-email-task/v1"
    for field in fields:
        encoded = field.encode()
        payload += len(encoded).to_bytes(8, "big") + encoded
    message = EmailMessage(policy=SMTP)
    message["From"] = sender
    message["To"] = mailbox
    message["Subject"] = subject
    message["X-Gosling-Timestamp"] = timestamp
    message["X-Gosling-Nonce"] = nonce
    message["X-Gosling-Signature"] = base64.b64encode(hmac.digest(key, payload, "sha256")).decode()
    message.set_content(body)
    sys.stdout.buffer.write(message.as_bytes())


if __name__ == "__main__":
    main()
