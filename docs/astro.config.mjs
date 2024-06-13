import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

import react from "@astrojs/react";

// https://astro.build/config
export default defineConfig({
    site: "https://mierak.github.io",
    base: "/rmpc",
    build: {
        format: "directory",
    },
    integrations: [
        starlight({
            title: "rmpc wiki",
            social: {
                github: "https://github.com/mierak/rmpc",
            },
            sidebar: [
                {
                    label: "Overview",
                    link: "overview",
                },
                {
                    label: "Guides",
                    autogenerate: {
                        directory: "guides",
                    },
                },
                {
                    label: "Configuration",
                    autogenerate: {
                        directory: "configuration",
                    },
                },
                {
                    label: "Reference",
                    autogenerate: {
                        directory: "reference",
                    },
                },
                {
                    label: "Theme Gallery",
                    autogenerate: {
                        directory: "themes",
                    },
                },
            ],
            customCss: ["./src/styles/custom.css"],
            components: {
                Hero: "./src/components/Hero.astro",
            },
        }),
        react(),
    ],
});
