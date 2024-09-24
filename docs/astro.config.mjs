import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLinksValidator from "starlight-links-validator";

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
            title: "Rmpc",
            plugins: [starlightLinksValidator()],
            editLink: {
                baseUrl: "https://github.com/mierak/rmpc/edit/master/docs/",
            },
            social: {
                github: "https://github.com/mierak/rmpc",
            },
            sidebar: [
                {
                    label: "Overview",
                    link: "overview",
                },
                {
                    label: "Installation",
                    link: "installation",
                },
                {
                    label: "Try without installing",
                    link: "try-without-install",
                },
                {
                    label: "Configuration",
                    autogenerate: {
                        directory: "configuration",
                    },
                },
                {
                    label: "Guides",
                    autogenerate: {
                        directory: "guides",
                    },
                },
                {
                    label: "Reference",
                    autogenerate: {
                        directory: "reference",
                    },
                },
                {
                    label: "Theme gallery",
                    autogenerate: {
                        directory: "themes",
                    },
                },
            ],
            customCss: ["./src/styles/custom.css"],
            components: {
                Hero: "./src/components/Hero.astro",
                Header: "./src/components/Header.astro",
            },
        }),
        react(),
    ],
});
