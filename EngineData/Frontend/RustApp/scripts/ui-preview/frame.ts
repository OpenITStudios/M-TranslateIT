import "../../src/styles/app.css";
import { mount } from "svelte";
import FrameApp from "./FrameApp.svelte";

const target = document.querySelector<HTMLDivElement>("#ui-preview-frame");
if (!target) throw new Error("UI preview frame root not found.");

mount(FrameApp, { target });
