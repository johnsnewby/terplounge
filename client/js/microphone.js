async function getMicrophones() {
  let devices = await navigator.mediaDevices.enumerateDevices();
  let microphones = [];
  for (var device in devices.filter((device) => device.kind == "audioinput")) {
    microphones.push(device);
  }
  return microphones;
}

