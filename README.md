# Terplounge, a tool to practise oral interpretation

This repository contains the source code to Terplounge, a tool to allow solitary practise of simultaneous interpretation.

## Overview

The basic idea is that the user will listen to some spoken audio in one language (called the 'source' language), translate it into another ('target') language, and speak the translation out loud. A speech-to-text engine will transcribe the target audio, and at the end the user will be shown a comparison of the pre-existing translation, and their own.

## The architecture

![Architecture diagram](/doc/img/architecture.png "The Terploung architecture").

Terplounge is designed to be usable as a hosted product, or on your machine. Its core is a Rust program which contains a version of the Whisper speech-to-text engine which is optimised to run on normal computers. Users connect to this program, which contains a web server, and stream audio to it, which is converted to text and stored in a session (which is not persisted--i.e. it is gone when the program terminates). The program ships with a minimal interface contained within itself, which exposes the basic features of terplounge.

However the idea is that these simple components are just the start of what can be done. By building a dynamic web site around these core services a rich environment can be created.

By default the system will use Whisper.cpp for its transcription services. This is a

# Installation

## Installation steps

In order to run this, you will need a whisper model--currently hardcoded to 'medium'. Download it like this:

```
cd server
../scripts/download-ggml-model.sh [model_name]
```

These are the models available:

```
[
    "tiny.en", "tiny", "tiny-q5_1", "tiny.en-q5_1",
    "base.en", "base", "base-q5_1", "base.en-q5_1",
    "small.en", "small.en-tdrz", "small", "small-q5_1",
    "small.en-q5_1", "medium", "medium.en", "medium-q5_0",
    "medium.en-q5_0",  "large-v1", "large", "large-q5_0"
]
```

run the server from the `server` directory:

```
cargo run
```

## Environment variables100

```
WHISPER_THREADS=
LISTEN=
WHISPER_MODEL=
RUST_LOG=
RUST_BACKTRACE=
```

## Testing

open the file `websocket.html` in your browser, and hit start recording. If you are lucky you'll get a couple of seconds of transcription.
