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
                starlightLinksValidator({
                    exclude: [
                        "http://localhost:4321/rmpc", // a link to the local server in dev & contributing
                    ],
                }),
                starlightSidebarTopics([
                    {
                        id: "next",
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
                                label: "Development and contributing",
                                link: "development",
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
                        label: "Release (v0.8.0)",
                        link: "release/overview",
                        badge: { text: "Stable", variant: "default" },
                        icon: "seti:todo",
                        items: [
                            {
                                label: "Overview",
                                link: "release/overview",
                            },
                            {
                                label: "Installation",
                                link: "release/installation",
                            },
                            {
                                label: "Try without installing",
                                link: "release/try-without-install",
                            },
                            {
                                label: "Configuration",
                                autogenerate: {
                                    directory: "release/configuration",
                                },
                            },
                            {
                                label: "Guides",
                                autogenerate: {
                                    directory: "release/guides",
                                },
                            },
                            {
                                label: "Reference",
                                autogenerate: {
                                    directory: "release/reference",
                                },
                            },
                            {
                                label: "Theme gallery",
                                autogenerate: {
                                    directory: "release/themes",
                                },
                            },
                        ],
                    },
                    {
                        label: "Release (v0.7.0)",
                        link: "release-0-7-0/overview",
                        badge: { text: "Stable", variant: "default" },
                        icon: "seti:todo",
                        items: [
                            {
                                label: "Overview",
                                link: "release-0-7-0/overview",
                            },
                            {
                                label: "Installation",
                                link: "release-0-7-0/installation",
                            },
                            {
                                label: "Try without installing",
                                link: "release-0-7-0/try-without-install",
                            },
                            {
                                label: "Configuration",
                                autogenerate: {
                                    directory: "release-0-7-0/configuration",
                                },
                            },
                            {
                                label: "Guides",
                                autogenerate: {
                                    directory: "release-0-7-0/guides",
                                },
                            },
                            {
                                label: "Reference",
                                autogenerate: {
                                    directory: "release-0-7-0/reference",
                                },
                            },
                            {
                                label: "Theme gallery",
                                autogenerate: {
                                    directory: "release-0-7-0/themes",
                                },
                            },
                        ],
                    },
                ]),
            ],
            editLink: {
                baseUrl: "https://github.com/mierak/rmpc/edit/master/docs/",
            },
            social: [
                {
                    icon: "github",
                    label: "GitHub",
                    href: "https://github.com/mierak/rmpc",
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
