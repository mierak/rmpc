import { defineCollection } from "astro:content";
import { docsSchema } from "@astrojs/starlight/schema";
import { docsLoader } from "@astrojs/starlight/loaders";
import { topicSchema } from "starlight-sidebar-topics/schema";

export const collections = {
    docs: defineCollection({ loader: docsLoader(), schema: docsSchema({ extend: topicSchema }) }),
};
