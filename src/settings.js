const { invoke } = window.__TAURI__.core;

const input = document.querySelector("#api-key");
const saveButton = document.querySelector("#save");
const statusEl = document.querySelector("#status");
const autostartInput = document.querySelector("#autostart");

async function load() {
  const key = await invoke("get_api_key");
  if (key) input.value = key;
  autostartInput.checked = await invoke("get_autostart_enabled");
}

async function save() {
  await invoke("save_api_key", { key: input.value });
  statusEl.textContent = "저장됨";
  setTimeout(() => {
    statusEl.textContent = "";
  }, 1500);
}

load();
saveButton.addEventListener("click", save);
input.addEventListener("keydown", (event) => {
  if (event.key === "Enter") save();
});
autostartInput.addEventListener("change", () => {
  invoke("set_autostart_enabled", { enabled: autostartInput.checked });
});
