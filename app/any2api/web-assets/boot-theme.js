(function () {
  var stored;
  try {
    stored = localStorage.getItem("any2api-theme");
  } catch {
    stored = null;
  }
  var mode;
  if (stored === "light" || stored === "dark") {
    mode = stored;
  } else if (stored === "system") {
    mode = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  } else {
    mode = "light";
  }
  document.documentElement.dataset.theme = mode;
  document.documentElement.dataset.themeMode = mode;
  var themeColor = document.querySelector('meta[name="theme-color"]');
  if (themeColor) {
    themeColor.setAttribute("content", mode === "dark" ? "#0f1115" : "#f0f4f9");
  }
})();
