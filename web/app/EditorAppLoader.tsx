"use client";

import dynamic from "next/dynamic";

// `dynamic(..., { ssr: false })` isn't allowed directly inside a Server
// Component in this Next.js version — has to be called from within a
// Client Component, hence this one-line wrapper. EditorApp reads `window`
// directly during render (share-link display), which is only safe because
// it's never given an SSR pass at all.
const EditorApp = dynamic(() => import("@/app/EditorApp"), { ssr: false });

export default EditorApp;
