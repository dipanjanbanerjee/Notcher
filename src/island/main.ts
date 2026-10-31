import { mount } from "svelte";
import "../styles/base.css";
import "./island.css";
import Island from "./Island.svelte";

export default mount(Island, { target: document.getElementById("app")! });
