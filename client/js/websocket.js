import { Transcription } from "./transcription.js";

export let state = {
  bufferSize: 16384,
  AudioContext: undefined,
  context: undefined,
  processor: undefined,
  input: undefined,
  globalStream: undefined,
  theWebsocket: undefined,
  segmentCallback: undefined,
  disconnectCallback: undefined,
  transcription: new Transcription(),
  uuid: undefined,
  finalSequenceNumber: undefined,
  highestSequenceNumber: undefined,
};

export async function status() {
  let json = undefined;
  const res = await fetch(`/status/${state.uuid}`);
  try {
    json = await res.json();
  } catch (e) {
    return undefined;
  }
  console.log(`status=${JSON.stringify(json)}`);
  if (json.recording) {
    let download = document.getElementById("download");
    download.style.display = "block";
    let downloadLink = document.getElementById("download-link");
    downloadLink.href = `recordings/${json.uuid}`;
    downloadLink.download = "recording.wav";
  }
  return json;
}

export function resetTranscription() {
  let start = document.getElementById("start");
  start.textContent = "start";
  start.addEventListener("click", startTranscription);
  try {
    disconnect();
  } catch (e) {
    console.log(e);
  }
}

export function startStopping() {
  let start = document.getElementById("start");
  start.textContent = "stopping";
  start.removeEventListener("click", startTranscription);
  //start.addEventListener("click", resetTranscription);
  stopRecording();
  closeConnection().then(console.log("Sent stop request"));
}

export function startTranscription() {
  let inputSelect = document.getElementById("inputSource");
  let inputDevice = inputSelect.options[inputSelect.selectedIndex].value;
  connect(inputDevice);
  let lang = document.getElementById("lang");
  if (lang.value === "") {
    return;
  }
  let start = document.getElementById("start");
  start.textContent = "stop";
  start.removeEventListener("click", startTranscription);
  start.addEventListener("click", startStopping);
  updateProgress(0);
}

export function startPractice() {
  let inputSelect = document.getElementById("inputSource");
  let inputDevice = inputSelect.options[inputSelect.selectedIndex].value;
  connect(inputDevice);
  let lang = document.getElementById("lang");
  if (lang.value === "") {
    return;
  }
  let start = document.getElementById("start");
  connectToServer((segment) => {
    // state.transcription.addSegment(segment);
  });
  startRecording(inputDevice);
}

export function goToCompare(asset_id, lang) {
  if (! stage.uuid) {
    alert("Too soon!");
    return;
  }
  if (confirm("Are you sure you are done?")) {
    document.location = "/compare/" + asset_id + "/" + state.uuid + "/" + lang;
  }
}

function connectToServer(callbackFunction) {
  if (state.theWebsocket !== undefined) {
    console.log("state.theWebsocket not false: " + state.theWebsocket);
    return;
  }
  state.theWebsocket = initWebSocket(getWebSocketUri());
  if (state.theWebsocket === undefined) {
    alert("An error occured. Please notify the authorities");
    resetTranscription();
  }
  state.segmentCallback = callbackFunction;
}

function connect(deviceId) {
  console.log("connect()");
  connectToServer((segment) => {
    let content = document.getElementById("content");
    state.transcription.addSegment(segment);
    content.innerHTML = state.transcription.getText();
    content.scrollTop = content.scrollHeight;
  });
  startRecording(deviceId);
}

function disconnect() {
  if (!state.theWebsocket) {
    console.log("Attempt to disconnect when not connected");
    return;
  }
  state.theWebsocket.close();
  state.theWebsocket = undefined;
  console.log("Disconnected");
  if (state.disconnectCallback) {
    state.disconnectCallback();
  }
}

function getNativeSampleRate() {
  state.context = new AudioContext({
    latencyHint: "balanced",
  });
  return state.context.sampleRate;
}

function getWebSocketUri() {
  let lang = document.getElementById("lang").value;
  let sampleRate = getNativeSampleRate();
  let chat_path =
    "/chat?lang=" + lang + "&rate=" + sampleRate + "&uuid=" + state.uuid;

  let websocket_uri =
    window.location.protocol === "https:"
      ? "wss://" + window.location.host + chat_path
      : "ws://localhost:3030" + chat_path;

  return websocket_uri;
}

function getAppBaseUri() {
  return window.location.protocol === "https:" ||
    window.location.protocol === "http:"
    ? window.location.protocol + "//" + window.location.host
    : "http://localhost:3030";
}

export async function closeConnection() {
  console.log("in closeConnection()");
  await fetch(getAppBaseUri() + "/close/" + state.uuid, {
    method: "POST",
  });
}

//==================PROGRESS BAR==============
function updateProgress(percentage) {
  var elem = document.getElementById("progress-bar-inside");
  if(! elem) {
    return;
  }
  elem.style.width = percentage + "%";
  var textElem = document.getElementById("progress-bar-text");
  textElem.textContent = `Transcription progress: ${Math.ceil(percentage)}%`;
}

let sendfunction = function (e) {
  var left = e.inputBuffer.getChannelData(0);
  try {
    state.theWebsocket.send(left);
  } catch (e) {
    console.log(`Exception in sendfunction ${e}`);
    disconnect();
  }
};

//================= RECORDING =================
function startRecording(deviceId) {
  state.context = new AudioContext({
    latencyHint: "interactive",
  });
  state.processor = state.context.createScriptProcessor(state.bufferSize, 1, 1);
  state.processor.connect(state.context.destination);
  state.context.resume();

  console.log(JSON.stringify(state, null, 2));

  var handleSuccess = function (stream) {
    state.globalStream = stream;
    state.input = state.context.createMediaStreamSource(stream);
    state.input.connect(state.processor);

    state.processor.onaudioprocess = sendfunction;
  };
  navigator.mediaDevices
    .getUserMedia({ audio: true, video: false, deviceId: { exact: deviceId } })
    .then(handleSuccess);
}

async function stopRecording() {
  try {
    let track = state.globalStream.getTracks()[0];
    if (track) {
      track.stop();
    }
  } catch (e) {
    console.log(`Error stopping track ${e}`);
  }

  try {
    state.input.disconnect(state.processor);
    state.processor.disconnect(state.context.destination);
    state.context.close().then(function () {
      state.input = null;
      state.processor = null;
      state.context = null;
    });
  } catch (e) {
    console.log("Error disconnecting " + e);
  }
  const currentStatus = await status();
  state.finalSequenceNumber = currentStatus.sequence_number - 1;
  console.log(
    `Waiting for sequence number ${state.finalSequenceNumber} before closing websocket`,
  );
}

function initWebSocket(websocket_uri) {
  console.log(`Connecting to ${websocket_uri}`);
  try {
    let ws = new WebSocket(websocket_uri);
    ws.binaryType = "blob";

    ws.onopen = function () {
      console.log("connected");
    };

    ws.onclose = function () {
      disconnect();
    };

    ws.addEventListener("message", (e) => {
      let message = JSON.parse(e.data);
      console.log(message);
      state.uuid = message.uuid;
      if (!message.sequence_number) {
        // control message
        return;
      }
      state.segmentCallback(message);
      if (
        state.finalSequenceNumber &&
        state.finalSequenceNumber <= message.sequence_number
      ) {
        console.log("Last message received, disconnecting");
        const start = document.getElementById("start");
        start.textContent = "start";
        start.removeEventListener("click", startTranscription);
        start.addEventListener("click", resetTranscription);
        disconnect();
        return;
      }

      if (message.segment_number === 0) {
        status().then((s) => {
          if (s !== undefined) {
            const max = s.sequence_number;
            updateProgress((100 / max) * message.sequence_number);
            state.highestSequenceNumber = Math.max(
              state.highestSequenceNumber,
              s.sequence_number,
            );
          }
        });
      }
    });
    return ws;
  } catch (e) {
    console.log(`Error connecting websocket: ${e}`);
  }
}
