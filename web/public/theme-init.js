// Pre-paint theme + direction (blocking, same-origin: the production CSP forbids inline scripts).
;(function () {
  var root = document.documentElement
  try {
    var theme = localStorage.getItem("lp-theme") || "system"
    var dark =
      theme === "dark" ||
      (theme !== "light" && window.matchMedia("(prefers-color-scheme: dark)").matches)
    var query = new URLSearchParams(window.location.search).get("dir")
    var dir = query === "rtl" || query === "ltr" ? query : localStorage.getItem("lp-dir")
    root.classList.add(dark ? "dark" : "light")
    root.style.colorScheme = dark ? "dark" : "light"
    root.dir = dir === "rtl" ? "rtl" : "ltr"
    if (localStorage.getItem("lp-reduce-motion") === "1") root.dataset.reduceMotion = "true"
  } catch (e) {
    root.classList.add("dark")
    root.dir = "ltr"
  }
})()
