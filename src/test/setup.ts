import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// vitest runs without globals, so React Testing Library's own auto-cleanup
// never registers. Without this, renders leak between tests in the same file.
afterEach(cleanup);
