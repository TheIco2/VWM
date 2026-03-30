document.addEventListener("DOMContentLoaded", () => {
  const roots = [
    document.getElementById("settings-root"),
    document.getElementById("discover-root"),
    document.getElementById("editor-root"),
    document.getElementById("library-root"),
  ].filter(Boolean);

  roots.forEach((root) => {
    root.innerHTML = `
      <section class="panel">
        <h1>Window Manager Settings</h1>
        <p>Window Manager options are driven by <code>config.yaml</code> and <code>schema.yaml</code>.</p>
        <p>Discover, editor, and library surfaces are intentionally omitted until they have concrete behavior.</p>
      </section>
    `;
  });
});
