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
  } else {
    mode = "light";
  }
  document.documentElement.dataset.theme = mode;
  var themeColor = document.querySelector('meta[name="theme-color"]');
  if (themeColor) {
    themeColor.setAttribute("content", mode === "dark" ? "#0a0c10" : "#ffffff");
  }
})();
