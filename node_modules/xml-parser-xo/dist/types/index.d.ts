export declare type XmlParserOptions = {
    /**
     * Returns false to exclude a node. Default is true.
     */
    filter?: (node: XmlParserNode) => boolean | any;
    /**
     * True to throw an error when parsing XML document with invalid content like mismatched closing tags.
     */
    strictMode?: boolean;
};
export declare type XmlParserNodeType = 'Comment' | 'Text' | 'ProcessingInstruction' | 'Element' | 'DocumentType' | 'CDATA';
export declare type XmlParserNodeWrapper<T extends XmlParserNode> = {
    excluded: boolean;
    node: T;
};
export declare type XmlParserNode = {
    type: XmlParserNodeType;
};
export declare type XmlParserAttribute = {
    name: string;
    value: string;
};
export declare type XmlParserElementChildNode = XmlParserTextNode | XmlParserElementNode | XmlParserCDATANode | XmlParserCommentNode;
export declare type XmlParserDocumentChildNode = XmlParserDocumentTypeNode | XmlParserProcessingInstructionNode | XmlParserElementChildNode;
export declare type XmlParserProcessingInstructionNode = {
    type: 'ProcessingInstruction';
    name: string;
    attributes: Record<string, string>;
};
export declare type XmlParserElementNode = {
    type: 'Element';
    name: string;
    attributes: Record<string, string>;
    children: XmlParserElementChildNode[] | null;
};
export declare type XmlParserTextNode = {
    type: 'Text';
    content: string;
};
export declare type XmlParserCDATANode = {
    type: 'CDATA';
    content: string;
};
export declare type XmlParserCommentNode = {
    type: 'Comment';
    content: string;
};
export declare type XmlParserDocumentTypeNode = {
    type: 'DocumentType';
    content: string;
};
export declare type XmlParserResult = {
    declaration?: XmlParserProcessingInstructionNode | null;
    root: XmlParserElementNode;
    children: XmlParserDocumentChildNode[];
};
export declare class ParsingError extends Error {
    readonly cause: string;
    constructor(message: string, cause: string);
}
/**
 * Parse the given XML string into an object.
 */
declare function parseXml(xml: string, options?: XmlParserOptions): XmlParserResult;
export default parseXml;
//# sourceMappingURL=index.d.ts.map