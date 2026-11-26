"use strict";

// Serialized into the webview. This navigates compiler-authored sections only;
// it never evaluates dialogue expressions or executes runtime effects.
function installReader(uri, labels, appliedMock, hasMock) {
  const input = document.getElementById("mock-context");
  const mockStatus = document.getElementById("mock-status");
  const updateMockStatus = () => {
    mockStatus.textContent = appliedMock !== null && input.value !== appliedMock
      ? "Unapplied changes" : hasMock ? "Applied mock context" : "Authored view";
    if (appliedMock === null && input.value.trim() !== '{\n  "context": {},\n  "locals": {},\n  "args": {}\n}') {
      mockStatus.textContent = "Unapplied draft · authored view";
    }
  };
  input.addEventListener("input", updateMockStatus);
  updateMockStatus();
  if (!labels.length || !document.getElementById("view-read")) return;

  const saved = vscode.getState()?.dialogueReader;
  const signature = JSON.stringify(labels);
  const validIndex = value => Number.isInteger(value) && value >= 0 && value < labels.length;
  const restored = saved?.uri === uri && saved.signature === signature;
  let history = restored && Array.isArray(saved.history) ? saved.history.filter(validIndex).slice(-100) : [0];
  if (!history.length) history = [0];
  let view = restored && saved.view === "flow" ? "flow" : "read";
  const layout = document.querySelector(".reader-layout");
  const picker = document.getElementById("section-picker");
  const map = document.querySelector(".dialogue-map");
  const mockPanel = document.querySelector(".mock-controls");
  if (restored && saved.mockOpen) mockPanel.open = true;
  const save = () => vscode.setState({ ...vscode.getState(), dialogueReader: {
    uri, signature, history, view, mockOpen: mockPanel.open,
  } });
  mockPanel.addEventListener("toggle", save);

  function render(focus = false) {
    const current = history[history.length - 1];
    document.querySelectorAll("[data-section]").forEach(section => {
      section.hidden = Number(section.dataset.section) !== current;
    });
    document.querySelectorAll("[data-select-section]").forEach(button => {
      button.setAttribute("aria-current", String(Number(button.dataset.selectSection) === current));
    });
    picker.value = String(current);
    document.getElementById("reader-back").disabled = history.length < 2;
    document.getElementById("view-read").setAttribute("aria-pressed", String(view === "read"));
    document.getElementById("view-flow").setAttribute("aria-pressed", String(view === "flow"));
    document.getElementById("flow-pane").hidden = view !== "flow";
    layout.classList.toggle("flow-view", view === "flow");
    if (map) map.open = view === "flow";
    const trail = document.getElementById("reader-history");
    trail.replaceChildren();
    history.slice(-8).forEach((index, offset) => {
      const position = Math.max(0, history.length - 8) + offset;
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = labels[index];
      button.title = "Return to visited section " + labels[index];
      button.setAttribute("aria-current", String(position === history.length - 1));
      button.addEventListener("click", () => {
        history = history.slice(0, position + 1);
        render(true);
      });
      trail.append(button);
    });
    document.dispatchEvent(new CustomEvent("dialogue-section-selected", { detail: current }));
    save();
    if (focus) document.querySelector(`[data-section="${current}"] h2`)?.focus();
  }
  function follow(index) {
    if (!validIndex(index)) return;
    history.push(index);
    history = history.slice(-100);
    render(true);
  }
  document.querySelectorAll("[data-follow]").forEach(button => {
    button.addEventListener("click", () => follow(Number(button.dataset.follow)));
  });
  document.querySelectorAll("[data-select-section]").forEach(button => {
    button.addEventListener("click", () => follow(Number(button.dataset.selectSection)));
  });
  picker.addEventListener("change", () => follow(Number(picker.value)));
  document.getElementById("reader-back").addEventListener("click", () => {
    if (history.length > 1) history.pop();
    render(true);
  });
  document.getElementById("reader-restart").addEventListener("click", () => {
    history = [history[history.length - 1]];
    render(true);
  });
  for (const mode of ["read", "flow"]) {
    document.getElementById("view-" + mode).addEventListener("click", () => {
      view = mode;
      render();
    });
  }
  render();
}

module.exports = { installReader };
