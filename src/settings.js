const { invoke } = window.__TAURI__.core;

const input = document.querySelector("#api-key");
const saveButton = document.querySelector("#save");
const statusEl = document.querySelector("#status");

async function load() {
  const key = await invoke("get_api_key");
  if (key) input.value = key;
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
