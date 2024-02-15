
async function getMicrophones() {
  let devices = await navigator.mediaDevices.enumerateDevices();
  let microphones = [];
  for (var device in devices.filter((device) => device.kind == "audioinput")) {
    microphones.push(device);
  }
  return microphones;
}

export async function populateMicrophones() {
  let microphoneSelect = document.getElementById("inputSource");
  try {
    await navigator.mediaDevices.getUserMedia({ audio: true, video: false });
  } catch(e) {
    console.log(`Couldn't get user media: ${e}`);
    throw(e);
  }
  console.log("Before enumerateDevices");
  let microphones = await navigator.mediaDevices.enumerateDevices();
  console.log("After enumerateDevices");
  console.log(JSON.stringify(microphones));
  if (microphones.length > 0) {
    for (var microphone of microphones) {
      console.log(JSON.stringify(microphone));
      if (microphone.label === "") {
        continue;
      }
      let option = document.createElement("option");
      option.text =
        microphone.label.length < 30
          ? microphone.label
          : microphone.label.substring(0, 15) + "...";
      option.title = microphone.label;
      option.value = microphone.deviceId;
      microphoneSelect.add(option);
    }
  }
}
