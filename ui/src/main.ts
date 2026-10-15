import { mount } from "svelte";
import App from "./App.svelte";
import "./lib/tokens.css";
import { restoreAppearance } from "./lib/api";

restoreAppearance();

export default mount(App, { target: document.getElementById("app")! });
