(function(f){if(typeof exports==="object"&&typeof module!=="undefined"){module.exports=f()}else if(typeof define==="function"&&define.amd){define([],f)}else{var g;if(typeof window!=="undefined"){g=window}else if(typeof global!=="undefined"){g=global}else if(typeof self!=="undefined"){g=self}else{g=this}g.xmlFormatter = f()}})(function(){var define,module,exports;return (function(){function r(e,n,t){function o(i,f){if(!n[i]){if(!e[i]){var c="function"==typeof require&&require;if(!f&&c)return c(i,!0);if(u)return u(i,!0);var a=new Error("Cannot find module '"+i+"'");throw a.code="MODULE_NOT_FOUND",a}var p=n[i]={exports:{}};e[i][0].call(p.exports,function(r){var n=e[i][1][r];return o(n||r)},p,p.exports,r,e,n,t)}return n[i].exports}for(var u="function"==typeof require&&require,i=0;i<t.length;i++)o(t[i]);return o}return r})()({1:[function(require,module,exports){
"use strict";

function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
function _classCallCheck(a, n) { if (!(a instanceof n)) throw new TypeError("Cannot call a class as a function"); }
function _inherits(t, e) { if ("function" != typeof e && null !== e) throw new TypeError("Super expression must either be null or a function"); t.prototype = Object.create(e && e.prototype, { constructor: { value: t, writable: !0, configurable: !0 } }), Object.defineProperty(t, "prototype", { writable: !1 }), e && _setPrototypeOf(t, e); }
function _createSuper(t) { var r = _isNativeReflectConstruct(); return function () { var e, o = _getPrototypeOf(t); if (r) { var s = _getPrototypeOf(this).constructor; e = Reflect.construct(o, arguments, s); } else e = o.apply(this, arguments); return _possibleConstructorReturn(this, e); }; }
function _possibleConstructorReturn(t, e) { if (e && ("object" == _typeof(e) || "function" == typeof e)) return e; if (void 0 !== e) throw new TypeError("Derived constructors may only return object or undefined"); return _assertThisInitialized(t); }
function _assertThisInitialized(e) { if (void 0 === e) throw new ReferenceError("this hasn't been initialised - super() hasn't been called"); return e; }
function _wrapNativeSuper(t) { var r = "function" == typeof Map ? new Map() : void 0; return _wrapNativeSuper = function _wrapNativeSuper(t) { if (null === t || !_isNativeFunction(t)) return t; if ("function" != typeof t) throw new TypeError("Super expression must either be null or a function"); if (void 0 !== r) { if (r.has(t)) return r.get(t); r.set(t, Wrapper); } function Wrapper() { return _construct(t, arguments, _getPrototypeOf(this).constructor); } return Wrapper.prototype = Object.create(t.prototype, { constructor: { value: Wrapper, enumerable: !1, writable: !0, configurable: !0 } }), _setPrototypeOf(Wrapper, t); }, _wrapNativeSuper(t); }
function _construct(t, e, r) { if (_isNativeReflectConstruct()) return Reflect.construct.apply(null, arguments); var o = [null]; o.push.apply(o, e); var p = new (t.bind.apply(t, o))(); return r && _setPrototypeOf(p, r.prototype), p; }
function _isNativeReflectConstruct() { try { var t = !Boolean.prototype.valueOf.call(Reflect.construct(Boolean, [], function () {})); } catch (t) {} return (_isNativeReflectConstruct = function _isNativeReflectConstruct() { return !!t; })(); }
function _isNativeFunction(t) { try { return -1 !== Function.toString.call(t).indexOf("[native code]"); } catch (n) { return "function" == typeof t; } }
function _setPrototypeOf(t, e) { return _setPrototypeOf = Object.setPrototypeOf ? Object.setPrototypeOf.bind() : function (t, e) { return t.__proto__ = e, t; }, _setPrototypeOf(t, e); }
function _getPrototypeOf(t) { return _getPrototypeOf = Object.setPrototypeOf ? Object.getPrototypeOf.bind() : function (t) { return t.__proto__ || Object.getPrototypeOf(t); }, _getPrototypeOf(t); }
Object.defineProperty(exports, "__esModule", {
  value: true
});
exports.ParsingError = void 0;
var ParsingError = /*#__PURE__*/function (_Error) {
  _inherits(ParsingError, _Error);
  var _super = _createSuper(ParsingError);
  function ParsingError(message, cause) {
    var _this;
    _classCallCheck(this, ParsingError);
    _this = _super.call(this, message);
    _this.cause = cause;
    return _this;
  }
  return ParsingError;
}( /*#__PURE__*/_wrapNativeSuper(Error));
exports.ParsingError = ParsingError;
var parsingState;
function nextChild() {
  return element(false) || text() || comment() || cdata() || processingInstruction();
}
function nextRootChild() {
  match(/\s*/);
  return element(true) || comment() || doctype() || processingInstruction();
}
function parseDocument() {
  var declaration = processingInstruction();
  var children = [];
  var documentRootNode;
  var child = nextRootChild();
  while (child) {
    if (child.node.type === 'Element') {
      if (documentRootNode) {
        throw new Error('Found multiple root nodes');
      }
      documentRootNode = child.node;
    }
    if (!child.excluded) {
      children.push(child.node);
    }
    child = nextRootChild();
  }
  if (!documentRootNode) {
    throw new ParsingError('Failed to parse XML', 'Root Element not found');
  }
  if (parsingState.xml.length !== 0) {
    throw new ParsingError('Failed to parse XML', 'Not Well-Formed XML');
  }
  return {
    declaration: declaration ? declaration.node : null,
    root: documentRootNode,
    children: children
  };
}
function processingInstruction() {
  var m = match(/^<\?([\w-:.]+)\s*/);
  if (!m) return;
  // tag
  var node = {
    name: m[1],
    type: 'ProcessingInstruction',
    content: ''
  };
  var endMarkerIndex = parsingState.xml.indexOf('?>');
  if (endMarkerIndex > -1) {
    node.content = parsingState.xml.substring(0, endMarkerIndex).trim();
    parsingState.xml = parsingState.xml.slice(endMarkerIndex);
  } else {
    throw new ParsingError('Failed to parse XML', 'ProcessingInstruction closing tag not found');
  }
  match(/\?>/);
  return {
    excluded: parsingState.options.filter(node) === false,
    node: node
  };
}
function element(matchRoot) {
  var m = match(/^<([^?!</>\s]+)\s*/);
  if (!m) return;
  // name
  var node = {
    type: 'Element',
    name: m[1],
    attributes: {},
    children: []
  };
  var excluded = matchRoot ? false : parsingState.options.filter(node) === false;
  // attributes
  while (!(eos() || is('>') || is('?>') || is('/>'))) {
    var attr = attribute();
    if (attr) {
      node.attributes[attr.name] = attr.value;
    } else {
      return;
    }
  }
  // self closing tag
  if (match(/^\s*\/>/)) {
    node.children = null;
    return {
      excluded: excluded,
      node: node
    };
  }
  match(/\??>/);
  // children
  var child = nextChild();
  while (child) {
    if (!child.excluded) {
      node.children.push(child.node);
    }
    child = nextChild();
  }
  // closing
  if (parsingState.options.strictMode) {
    var closingTag = "</".concat(node.name, ">");
    if (parsingState.xml.startsWith(closingTag)) {
      parsingState.xml = parsingState.xml.slice(closingTag.length);
    } else {
      throw new ParsingError('Failed to parse XML', "Closing tag not matching \"".concat(closingTag, "\""));
    }
  } else {
    match(/^<\/[\w-:.\u00C0-\u00FF]+\s*>/);
  }
  return {
    excluded: excluded,
    node: node
  };
}
function doctype() {
  var m = match(/^<!DOCTYPE\s+\S+\s+SYSTEM[^>]*>/) || match(/^<!DOCTYPE\s+\S+\s+PUBLIC[^>]*>/) || match(/^<!DOCTYPE\s+\S+\s*\[[^\]]*]>/) || match(/^<!DOCTYPE\s+\S+\s*>/);
  if (m) {
    var node = {
      type: 'DocumentType',
      content: m[0]
    };
    return {
      excluded: parsingState.options.filter(node) === false,
      node: node
    };
  }
}
function cdata() {
  if (parsingState.xml.startsWith('<![CDATA[')) {
    var endPositionStart = parsingState.xml.indexOf(']]>');
    if (endPositionStart > -1) {
      var endPositionFinish = endPositionStart + 3;
      var node = {
        type: 'CDATA',
        content: parsingState.xml.substring(0, endPositionFinish)
      };
      parsingState.xml = parsingState.xml.slice(endPositionFinish);
      return {
        excluded: parsingState.options.filter(node) === false,
        node: node
      };
    }
  }
}
function comment() {
  var m = match(/^<!--[\s\S]*?-->/);
  if (m) {
    var node = {
      type: 'Comment',
      content: m[0]
    };
    return {
      excluded: parsingState.options.filter(node) === false,
      node: node
    };
  }
}
function text() {
  var m = match(/^([^<]+)/);
  if (m) {
    var node = {
      type: 'Text',
      content: m[1]
    };
    return {
      excluded: parsingState.options.filter(node) === false,
      node: node
    };
  }
}
function attribute() {
  var m = match(/([^=]+)\s*=\s*("[^"]*"|'[^']*'|[^>\s]+)\s*/);
  if (m) {
    return {
      name: m[1].trim(),
      value: stripQuotes(m[2].trim())
    };
  }
}
function stripQuotes(val) {
  return val.replace(/^['"]|['"]$/g, '');
}
/**
 * Match `re` and advance the string.
 */
function match(re) {
  var m = parsingState.xml.match(re);
  if (m) {
    parsingState.xml = parsingState.xml.slice(m[0].length);
    return m;
  }
}
/**
 * End-of-source.
 */
function eos() {
  return 0 === parsingState.xml.length;
}
/**
 * Check for `prefix`.
 */
function is(prefix) {
  return 0 === parsingState.xml.indexOf(prefix);
}
/**
 * Parse the given XML string into an object.
 */
function parseXml(xml) {
  var options = arguments.length > 1 && arguments[1] !== undefined ? arguments[1] : {};
  xml = xml.trim();
  var filter = options.filter || function () {
    return true;
  };
  parsingState = {
    xml: xml,
    options: Object.assign(Object.assign({}, options), {
      filter: filter,
      strictMode: options.strictMode === true
    })
  };
  return parseDocument();
}
if (typeof module !== 'undefined' && (typeof exports === "undefined" ? "undefined" : _typeof(exports)) === 'object') {
  module.exports = parseXml;
}
exports["default"] = parseXml;

},{}],"xml-formatter":[function(require,module,exports){
"use strict";

function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
var __importDefault = void 0 && (void 0).__importDefault || function (mod) {
  return mod && mod.__esModule ? mod : {
    "default": mod
  };
};
Object.defineProperty(exports, "__esModule", {
  value: true
});
var xml_parser_xo_1 = __importDefault(require("xml-parser-xo"));
function newLine(state) {
  if (!state.options.indentation && !state.options.lineSeparator) return;
  state.content += state.options.lineSeparator;
  var i;
  for (i = 0; i < state.level; i++) {
    state.content += state.options.indentation;
  }
}
function indent(state) {
  state.content = state.content.replace(/ +$/, '');
  var i;
  for (i = 0; i < state.level; i++) {
    state.content += state.options.indentation;
  }
}
function appendContent(state, content) {
  state.content += content;
}
function processNode(node, state, preserveSpace) {
  if (node.type === 'Element') {
    processElementNode(node, state, preserveSpace);
  } else if (node.type === 'ProcessingInstruction') {
    processProcessingIntruction(node, state);
  } else if (typeof node.content === 'string') {
    processContent(node.content, state, preserveSpace);
  } else {
    throw new Error('Unknown node type: ' + node.type);
  }
}
function processContent(content, state, preserveSpace) {
  if (!preserveSpace) {
    var trimmedContent = content.trim();
    if (state.options.lineSeparator) {
      content = trimmedContent;
    } else if (trimmedContent.length === 0) {
      content = trimmedContent;
    }
  }
  if (content.length > 0) {
    if (!preserveSpace && state.content.length > 0) {
      newLine(state);
    }
    appendContent(state, content);
  }
}
function isPathMatchingIgnoredPaths(path, ignoredPaths) {
  var fullPath = '/' + path.join('/');
  var pathLastPart = path[path.length - 1];
  return ignoredPaths.includes(pathLastPart) || ignoredPaths.includes(fullPath);
}
function processElementNode(node, state, preserveSpace) {
  state.path.push(node.name);
  if (!preserveSpace && state.content.length > 0) {
    newLine(state);
  }
  appendContent(state, '<' + node.name);
  processAttributes(state, node.attributes);
  if (node.children === null || state.options.forceSelfClosingEmptyTag && node.children.length === 0) {
    var selfClosingNodeClosingTag = state.options.whiteSpaceAtEndOfSelfclosingTag ? ' />' : '/>';
    // self-closing node
    appendContent(state, selfClosingNodeClosingTag);
  } else if (node.children.length === 0) {
    // empty node
    appendContent(state, '></' + node.name + '>');
  } else {
    var nodeChildren = node.children;
    appendContent(state, '>');
    state.level++;
    var nodePreserveSpace = node.attributes['xml:space'] === 'preserve' || preserveSpace;
    var ignoredPath = false;
    if (!nodePreserveSpace && state.options.ignoredPaths) {
      ignoredPath = isPathMatchingIgnoredPaths(state.path, state.options.ignoredPaths);
      nodePreserveSpace = ignoredPath;
    }
    if (!nodePreserveSpace && state.options.collapseContent) {
      var containsTextNodes = false;
      var containsTextNodesWithLineBreaks = false;
      var containsNonTextNodes = false;
      nodeChildren.forEach(function (child, index) {
        if (child.type === 'Text') {
          if (child.content.includes('\n')) {
            containsTextNodesWithLineBreaks = true;
            child.content = child.content.trim();
          } else if ((index === 0 || index === nodeChildren.length - 1) && !preserveSpace) {
            if (child.content.trim().length === 0) {
              // If the text node is at the start or end and is empty, it should be ignored when formatting
              child.content = '';
            }
          }
          // If there is some content or whitespaces have been removed and there is no other siblings
          if (child.content.trim().length > 0 || nodeChildren.length === 1) {
            containsTextNodes = true;
          }
        } else if (child.type === 'CDATA') {
          containsTextNodes = true;
        } else {
          containsNonTextNodes = true;
        }
      });
      if (containsTextNodes && (!containsNonTextNodes || !containsTextNodesWithLineBreaks)) {
        nodePreserveSpace = true;
      }
    }
    nodeChildren.forEach(function (child) {
      processNode(child, state, preserveSpace || nodePreserveSpace);
    });
    state.level--;
    if (!preserveSpace && !nodePreserveSpace) {
      newLine(state);
    }
    if (ignoredPath) {
      indent(state);
    }
    appendContent(state, '</' + node.name + '>');
  }
  state.path.pop();
}
function processAttributes(state, attributes) {
  Object.keys(attributes).forEach(function (attr) {
    var escaped = attributes[attr].replace(/"/g, '&quot;');
    appendContent(state, ' ' + attr + '="' + escaped + '"');
  });
}
function processProcessingIntruction(node, state) {
  if (state.content.length > 0) {
    newLine(state);
  }
  appendContent(state, '<?' + node.name);
  appendContent(state, ' ' + node.content.trim());
  appendContent(state, '?>');
}
/**
 * Converts the given XML into human readable format.
 */
function formatXml(xml) {
  var options = arguments.length > 1 && arguments[1] !== undefined ? arguments[1] : {};
  options.indentation = 'indentation' in options ? options.indentation : '    ';
  options.collapseContent = options.collapseContent === true;
  options.lineSeparator = 'lineSeparator' in options ? options.lineSeparator : '\r\n';
  options.whiteSpaceAtEndOfSelfclosingTag = options.whiteSpaceAtEndOfSelfclosingTag === true;
  options.throwOnFailure = options.throwOnFailure !== false;
  try {
    var parsedXml = (0, xml_parser_xo_1["default"])(xml, {
      filter: options.filter,
      strictMode: options.strictMode
    });
    var state = {
      content: '',
      level: 0,
      options: options,
      path: []
    };
    if (parsedXml.declaration) {
      processProcessingIntruction(parsedXml.declaration, state);
    }
    parsedXml.children.forEach(function (child) {
      processNode(child, state, false);
    });
    if (!options.lineSeparator) {
      return state.content;
    }
    return state.content.replace(/\r\n/g, '\n').replace(/\n/g, options.lineSeparator);
  } catch (err) {
    if (options.throwOnFailure) {
      throw err;
    }
    return xml;
  }
}
formatXml.minify = function (xml) {
  var options = arguments.length > 1 && arguments[1] !== undefined ? arguments[1] : {};
  return formatXml(xml, Object.assign(Object.assign({}, options), {
    indentation: '',
    lineSeparator: ''
  }));
};
if (typeof module !== 'undefined' && (typeof exports === "undefined" ? "undefined" : _typeof(exports)) === 'object') {
  module.exports = formatXml;
}
exports["default"] = formatXml;

},{"xml-parser-xo":1}]},{},[])("xml-formatter")
});
