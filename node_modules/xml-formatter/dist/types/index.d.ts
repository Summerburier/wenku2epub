import { XmlParserNode } from 'xml-parser-xo';
export declare type XMLFormatterOptions = {
    /**
     * List of XML element paths to ignore during formatting.
     * This can be a partial path (element tag name) or full path starting from the document element.
     * e.g. ['/html/head/script', 'pre']
     */
    ignoredPaths?: string[];
    /**
     * The value used for indentation.
     * Default = '    '
     */
    indentation?: string;
    /**
     * Return false to exclude the node.
     */
    filter?: (node: XmlParserNode) => boolean;
    /**
     * True to keep content in the same line as the element.
     * Notes: Only works if element contains at least one text node.
     * Default: false
     */
    collapseContent?: boolean;
    /**
     * The line separator to use.
     * Default: '\r\n'
     */
    lineSeparator?: string;
    /**
     * To either end ad self closing tag with `<tag/>` or `<tag />`.
     * Default: false
     */
    whiteSpaceAtEndOfSelfclosingTag?: boolean;
    /**
     * Throw an error when XML fails to parse and get formatted.
     * Notes: If set to `false`, the original XML is returned when an error occurs.
     * Default: true
     */
    throwOnFailure?: boolean;
    /**
     * True to throw an error when parsing XML document with invalid content like mismatched closing tags.
     */
    strictMode?: boolean;
    /**
     * True to force empty tags to be self-closing.
     */
    forceSelfClosingEmptyTag?: boolean;
};
export declare type XMLFormatterMinifyOptions = Omit<XMLFormatterOptions, 'lineSeparator' | 'indentation'>;
/**
 * Converts the given XML into human readable format.
 */
declare function formatXml(xml: string, options?: XMLFormatterOptions): string;
declare namespace formatXml {
    var minify: (xml: string, options?: XMLFormatterMinifyOptions) => string;
}
export default formatXml;
//# sourceMappingURL=index.d.ts.map