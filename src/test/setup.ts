import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// vitest 未开 globals 时 RTL 不会自动清理 DOM，测试间会互相污染查询结果
afterEach(cleanup);
