import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLinksValidator from "starlight-links-validator";
import starlightSidebarTopics from "starlight-sidebar-topics";

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
            title: "rmpc",
            plugins: [
                starlightLinksValidator(),
                starlightSidebarTopics([
                    {
                        label: "Latest git",
                        link: "next/overview",
                        icon: "seti:git",
                        badge: { text: "Dev", variant: "caution" },
                        items: [
                            {
                                label: "Overview",
                                link: "next/overview",
                            },
                            {
                                label: "Installation",
                                link: "next/installation",
                            },
                            {
                                label: "Try without installing",
                                link: "next/try-without-install",
                            },
                            {
                                label: "Configuration",
                                autogenerate: {
                                    directory: "next/configuration",
                                },
                            },
                            {
                                label: "Guides",
                                autogenerate: {
                                    directory: "next/guides",
                                },
                            },
                            {
                                label: "Reference",
                                autogenerate: {
                                    directory: "next/reference",
                                },
                            },
                            {
                                label: "Theme gallery",
                                autogenerate: {
                                    directory: "next/themes",
                                },
                            },
                        ],
                    },
                    {
                        label: "Release (v0.7.0)",
                        link: "latest/overview",
                        badge: { text: "Stable", variant: "default" },
                        icon: "seti:todo",
                        items: [
                            {
                                label: "Overview",
                                link: "latest/overview",
                            },
                            {
                                label: "Installation",
                                link: "latest/installation",
                            },
                            {
                                label: "Try without installing",
                                link: "latest/try-without-install",
                            },
                            {
                                label: "Configuration",
                                autogenerate: {
                                    directory: "latest/configuration",
                                },
                            },
                            {
                                label: "Guides",
                                autogenerate: {
                                    directory: "latest/guides",
                                },
                            },
                            {
                                label: "Reference",
                                autogenerate: {
                                    directory: "latest/reference",
                                },
                            },
                            {
                                label: "Theme gallery",
                                autogenerate: {
                                    directory: "latest/themes",
                                },
                            },
                        ],
                    },
                ]),
            ],
            editLink: {
                baseUrl: "https://github.com/mierak/rmpc/edit/master/docs/",
            },
            social: {
                github: "https://github.com/mierak/rmpc",
            },
            customCss: ["./src/styles/custom.css"],
            components: {
                Hero: "./src/components/Hero.astro",
                Header: "./src/components/Header.astro",
            },
        }),
        react(),
    ],
});
