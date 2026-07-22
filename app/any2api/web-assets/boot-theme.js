(function () {
  var stored;
  try {
    stored = localStorage.getItem("any2api-theme");
  } catch {
    stored = null;
  }
  var mode = stored === "light" || stored === "dark" ? stored : "system";
  var dark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  var resolved = mode === "system" ? (dark ? "dark" : "light") : mode;
  document.documentElement.dataset.theme = resolved;
  document.documentElement.dataset.themeMode = mode;
  var themeColor = document.querySelector('meta[name="theme-color"]');
  if (themeColor) {
    themeColor.setAttribute("content", resolved === "dark" ? "#0f1115" : "#f0f4f9");
  }
})();
