// Distinct Vite HTML entry; production src/main.ts and Tauri startup are untouched.
import "../../src/styles/app.css";
import { mount } from "svelte";
import PreviewApp from "./PreviewApp.svelte";

const target = document.querySelector<HTMLDivElement>("#ui-preview");
if (!target) throw new Error("UI preview root not found.");

mount(PreviewApp, { target });
