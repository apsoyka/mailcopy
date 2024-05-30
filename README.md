# Introduction

MailCopy is a command-line program which performs a complete backup of all mailboxes and messages from an email provider using the IMAP protocol.

# Usage

The following examples show how to use the program.

## SSL/TLS

```bash
mailcopy localhost 993 localhost.tar.zst
```

## STARTTLS & Accept Untrusted Certificate

```bash
mailcopy -t -i localhost 143 localhost.tar.zst
```

# Authentication

A username and password must be provided to the program through one of the following methods: program arguments, a `.env` file, or environment variables.
