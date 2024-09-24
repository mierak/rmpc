type Props = {
    name?: string;
    type: "string" | "number" | "boolean" | "other" | string[];
    customText?: string;
    optional?: boolean;
    default?: string;
    link?: string;
};

export default function ConfigValue(props: Props) {
    const { name, type, customText, optional, link } = props;
    let value;
    let clazz;
    if (type === "string") {
        value = customText ?? `"<${type}>"`;
        clazz = "string";
    } else if (type === "boolean") {
        value = customText ?? `true | false`;
        clazz = "bool";
    } else if (type === "number") {
        value = customText ?? `<${type}>`;
        clazz = "number";
    } else if (Array.isArray(type)) {
        value = type.join(" | ");
        clazz = "other";
    } else {
        value = customText ?? `<${type}>`;
        clazz = "other";
    }

    const valuediv = link ? (
        <a href={link}>
            <span className={clazz}>{value}</span>
        </a>
    ) : (
        <span className={clazz}>{value}</span>
    );
    // {
    //     defaultValue && (
    //         <div class="code">
    //             <span class="prop">Default</span>: {defaultValue}
    //         </div>
    //     )
    // }

    return (
        <div className="container">
            {name ? (
                <div className="code">
                    <span className="prop">{name}</span>:{" "}
                    {optional ? <span className="other">None | Some({valuediv})</span> : valuediv}
                </div>
            ) : (
                <div className="code">
                    {optional ? <span className="other">None | Some({valuediv})</span> : valuediv}
                </div>
            )}
        </div>
    );
}
