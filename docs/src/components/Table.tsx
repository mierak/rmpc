import React, { useState } from "react";

type TableRow = {
    cells: string[];
    expandedContent?: string[];
};

type ExpandableTableProps = {
    headers: string[];
    rows: TableRow[];
};

function allBackticksToCodeBlock(input: string): string {
    let out = "";
    let inBacktick = false;
    for (const c of input) {
        if (c === "`") {
            inBacktick = !inBacktick;
            out += inBacktick ? '<code dir="auto">' : "</code>";
        } else {
            out += c;
        }
    }

    if (inBacktick) {
        throw new Error("Unmatched backticks in input string");
    }

    return out;
}

export const ExpandableTable: React.FC<ExpandableTableProps> = ({ headers, rows }) => {
    const [expandedRowId, setExpandedRowId] = useState<number[]>([]);

    const handleExpand = (rowId: number) => {
        setExpandedRowId((prevIds) => {
            if (prevIds.includes(rowId)) {
                return prevIds.filter((id) => id !== rowId);
            } else {
                return [...prevIds, rowId];
            }
        });
    };

    return (
        <table style={{ marginTop: "1rem" }}>
            <thead>
                <tr>
                    <th />
                    {headers.map((header, idx) => (
                        <th key={idx}>{header}</th>
                    ))}
                </tr>
            </thead>
            <tbody>
                {rows.map((row, idx) => {
                    const isExpanded = expandedRowId.includes(idx);
                    const rowStyle = isExpanded ? { borderBottom: "0px" } : {};
                    return (
                        <React.Fragment key={idx}>
                            <tr>
                                <td style={rowStyle}>
                                    {row.expandedContent && (
                                        <button
                                            aria-label={isExpanded ? "Collapse row" : "Expand row"}
                                            onClick={() => handleExpand(idx)}
                                            style={{
                                                background: "none",
                                                border: "none",
                                                cursor: "pointer",
                                                padding: "2px 6px",
                                                fontSize: "16px",
                                            }}
                                        >
                                            {isExpanded ? "−" : "+"}
                                        </button>
                                    )}
                                </td>
                                {row.cells.map((cell, idx) => (
                                    <td
                                        style={rowStyle}
                                        key={idx}
                                        dangerouslySetInnerHTML={{ __html: allBackticksToCodeBlock(cell) }}
                                    ></td>
                                ))}
                            </tr>
                            {isExpanded && row.expandedContent && typeof row.expandedContent == "string" && (
                                <tr>
                                    <td colSpan={headers.length + 1}>{row.expandedContent}</td>
                                </tr>
                            )}
                            {isExpanded && row.expandedContent && Array.isArray(row.expandedContent) && (
                                <tr>
                                    <td colSpan={headers.length + 1}>
                                        <ul>
                                            {row.expandedContent.map((content, contentIdx) => (
                                                <li
                                                    key={contentIdx}
                                                    dangerouslySetInnerHTML={{
                                                        __html: allBackticksToCodeBlock(content),
                                                    }}
                                                ></li>
                                            ))}
                                        </ul>
                                    </td>
                                </tr>
                            )}
                        </React.Fragment>
                    );
                })}
            </tbody>
        </table>
    );
};
