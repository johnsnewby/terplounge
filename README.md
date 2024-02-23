# Terplounge, a tool to practise oral interpretation

This repository contains the source code to Terplounge, a tool to allow solitary practise of simultaneous interpretation.

## Overview

Terplounge is a tool which allows simultaneous interpreters to practice alone. It works by having audio and video of people speaking in one language, and a set of one or more translation transcripts in other languages. The user speaks into their microphone and Terplounge automatically transcribes what they say, and compares it to the 'official' translation. It's important to note that the system doesn't judge the correctness or otherwise of the users' translation, it simply compares it to the official translation.

## Technical overview

The application consists of two parts, a server which exposes an API, and HTML/CSS/Javascript code which uses this API to provide a UX. The client-side (HTML) code serves two purposes: as an example of how to use the API, and as useable application in its own right. The front-end is bundled inside the server binary, making Terplounge usable by just downloading it to users' machines.

## The server

The backend ('server') is written in Rust, a high-performance language with memory-safety guarantees. The system is written to allow a choice of transcription engines--Whisper, an open-source speech-to-text system is bundled within the system, it performs relatively well on normal desktop hardware. The capability exists within the system for it to be used with commercial speech to text systems, or by integrating a system which uses a GPU to work more quickly.

Audio is sent to the server by calling the `/chat` endpoint and specifying the user's bit rate. Data is sent as a sequence of mono 32-bit floats (WAV format). The server responds initially with a JSON object with this sessions UUID:

```
{"uuid":"354f6692-8aa8-4d9e-aa84-766689c85146"}
```

followed by a sequence of chunks like this:
```
{"num_segments":3,"segment_end":720,"segment_number":0,"segment_start":0,"sequence_number":1,"translation":" Wir feiern heute nicht den Sieg einer Partei, sondern die Freiheit.","uuid":"5055d383-6b80-4427-9865-242f878c71bf"}
```
as the transcription proceeds.

After a period of 30 seconds in which no data is sent, the server side will automatically close the connection.

There are fundamentally two ways to use the server, although one doesn't need to choose one or the other. In the first, transcriptions are created which can be used to build up a library for users to practice with. In the second, the transcription is compared with a reference and the differences between the two are returned. In both cases the transcript itself and a WAV file of the user's audio are stored on the machine hosting the server.

In all cases the UUID returned by the websocket is used to identify the session. Apart from the inherent unguessability of the UUID there is no security implemented, the intention being that this would be provided by layers on top of the basic API, if needed.

The calls which can be made with the UUID are:

- `/chat?lang=XX&resource=YYY&rate=ZZZZ`

	`lang` is a 2-letter language code, for instance `de`. If it's not specified, the backend will attempt to guess it. `rate` defaults to 48,000. Optionally `resource` identifies a resource bundle, as described below.

- `/close/:uuid`
  marks the session for closure when all outstanding transcriptions have been completed.

- `/serve_resource/:resource_path`
	Returns the metadata of a resource. If the path begins with `/` then it will be interpreted as the exact path to a resource bundle, if not then it will be relative to the resource root, which is specified using the `RESOURCE_PATH` environment variable.

- `/status/:uuid`
Returns a JSON object in this form:

	```{"language":"","uuid":"2d82da3a-e2fc-4728-8c78-3f52481bfbe2","resource":null,"sample_rate":48000,"transcription_job_count":7,"transcription_completed_count":0}```

- The `transcription_job_count` here can be compared with the `transcription_completion_count` to get an idea of how the transcription process is proceedi
ng and give feedback to the user. There is sample code for theis in `server/templates/compare.html`.

- `/compare/:resource_id/:uuid/:lang`
Compares the transcript stored for this session (which may be incomplete, when transcription tasks are still running) with the reference transcript. The comparison is an array of objects, looking like this:

```
  {
    "change_type": "delete",
    "content": " "
  },
  {
    "change_type": "insert",
    "content": "Mitbürger!"
  },
  {
    "change_type": "insert",
    "content": "\n\n"
  },
  {
    "change_type": "equal",
    "content": "Wir"
  },
  {
    "change_type": "equal",
    "content": " "
  },
  {
    "change_type": "equal",
    "content": "feiern"
  },

```

## The client

The client is programmed in HTML5, CSS and vanilla Javascript. There are no external libraries used. The intention is that the code will remain valid and useful for as long as possible. The assets are included in the binary, so one possible use case for Terplounge is to be downloaded and run on the user's machine, making the software useful even in the absence of anyone hosting it on a server.

The basic entry point to the system is an index page showing the active sessions, and for each a link to its recording, its transcript and an HTML page which visualizes the changes between the user and reference transcripts. There is also a link to the transcript page, which has a useful button to copy the transcript to the clipboard.

### Internals

![Architecture diagram](/doc/img/architecture.png "The Terploung architecture").

The system works by having a central multiple-producer, multiple-consumer queue onto which segments of audio are posted from the websocket(s), and which return JSON containing the fragments of transcription. Each segment is identified by a session number, and a sequence number, which monotonically increases for each session from 0. When the input connection is severed and the number of segments equals the sequence number, the output connection is also severed. After this point the data are all still held in memory, enabling the transcript and comparison still to be performed.

The queuing system ensures that Terplounge will ultimately be able to process all audio, no matter how slowly.

The idea is that there will be several queue consumers, suiting different use cases. Currently whisper.cpp is used, as a base which works on almost all machines. On my laptop it's nowhere near real time; on a fast desktop machine it processes with about a 30 second lag.

### A guide to the source code files

`api.rs` provides the REST API, using the Warp server framework.
`compare.rs` uses the `similar` crate to perform comparison of the reference and user translations.
`dotfiles.rs` is not used currently
`error.rs` provides the `E<_>` result type, and the `Er` error type
`main.rs` has as little code in as possible
`metadata.rs` code to manipulate the resource bundles, described below
`queue.rs` functions to manipulate the queues.
`session.rs` session handling
`translate.rs` should be called `transcribe.rs`
`whispercpp.rs` the code which processes audio through `whisper.cpp` and receives text in retusn
`whisperx.rs` code to call an external whisperx server for greater throughput

## Resource bundles

A Terplounge resource bundle is a directory containing a set of files. One of these must be called `metadata.json` and look like this:

```
{
  "name": "John F Kennedy swearing-in ceremony and inaugural address, 20 January 1961",
  "url": "https://www.jfklibrary.org/asset-viewer/archives/JFKWHA/1961/JFKWHA-001/JFKWHA-001",
  "license": "US Govt",
  "audio": "main.mp4",
  "native": "en",
  "transcript": "en.txt",
  "translations":
    { "de": "de.txt" }
}
```

The fields have the following meaning

- `name` is the identifier which is presented to the user.
- `url` should if possible point to the source of the audio/video
- `license` is never spelled correctly, and identified the license which the work is used under
- `audio` can also mean video and must be a file in the form a browser can recognize and play
- `native` indicates the native language of the resource
- `transcript` is a transcript of the audio, if available
- `translations` is an object containing key-value pairs of language codes, and files in text format with the reference translation.

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
