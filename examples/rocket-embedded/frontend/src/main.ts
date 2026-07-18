import "./style.css";

const page = document.querySelector<HTMLScriptElement>("#app[data-page]");
const props = page?.dataset.page ? JSON.parse(page.dataset.page) : {};

document.querySelector<HTMLDivElement>("#app")!.textContent =
  props.props?.message ?? "Rocket + Inertia";
