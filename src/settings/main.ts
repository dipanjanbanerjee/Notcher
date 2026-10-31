import { mount } from "svelte";
import "../styles/base.css";
import Settings from "./Settings.svelte";

export default mount(Settings, { target: document.getElementById("app")! });
