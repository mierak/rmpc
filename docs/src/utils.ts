export type CodeItemType = "enum" | "struct";
export type CodeItem = {
    startLine: number;
    endLine: number;
};

export function findCodeItem(code: string, type: CodeItemType, name: string, rename = name): string {
    const lines = code.split("\n");

    let startLine = -1;
    let endLine = -1;
    for (let i = 0; i < lines.length; i++) {
        if (lines[i].includes(`${type} ${name} {`)) {
            startLine = i;
            break;
        }
    }

    if (startLine === -1) {
        throw new Error(`Cannot find ${name}.`);
    }

    let openBrackets = 1;
    let currentLineIdx = startLine + 1;
    while (openBrackets > 0) {
        for (const c of lines[currentLineIdx]) {
            if (c === "{") {
                openBrackets++;
            } else if (c === "}") {
                openBrackets--;
            }

            if (openBrackets === 0) {
                endLine = currentLineIdx;
                break;
            }
        }
        currentLineIdx++;
    }

    lines[startLine] = `${type} ${rename} {`;
    return lines.slice(startLine, endLine + 1).join("\n");
}
