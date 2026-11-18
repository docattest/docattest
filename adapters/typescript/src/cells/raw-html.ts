// What CommonMark reads instead of a code span when it starts first: an autolink, or an
// HTML tag, comment, processing instruction, declaration or CDATA section. The patterns are
// the spec's own.
const SPACE = '[ \\t\\n]';
const AUTOLINK = '<[A-Za-z][A-Za-z0-9+.-]{1,31}:[^<>\\x00-\\x20]*>';
const DOMAIN_LABEL = '[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?';
const EMAIL = `<[A-Za-z0-9.!#$%&'*+/=?^_\`{|}~-]+@${DOMAIN_LABEL}(?:\\.${DOMAIN_LABEL})*>`;
const TAG_NAME = '[A-Za-z][A-Za-z0-9-]*';
const ATTRIBUTE_VALUE = `(?:[^"'=<>\`\\x00-\\x20]+|'[^']*'|"[^"]*")`;
const ATTRIBUTE = `${SPACE}+[A-Za-z_:][A-Za-z0-9_.:-]*(?:${SPACE}*=${SPACE}*${ATTRIBUTE_VALUE})?`;
const OPEN_TAG = `<${TAG_NAME}(?:${ATTRIBUTE})*${SPACE}*/?>`;
const CLOSING_TAG = `</${TAG_NAME}${SPACE}*>`;
const COMMENT = '<!-->|<!--->|<!--[\\s\\S]*?-->';
const CDATA = '<!\\[CDATA\\[[\\s\\S]*?\\]\\]>';
const OTHER = `${COMMENT}|<\\?[\\s\\S]*?\\?>|<![A-Za-z][^>]*>|${CDATA}`;
const RAW = new RegExp(`^(?:${AUTOLINK}|${EMAIL}|${OPEN_TAG}|${CLOSING_TAG}|${OTHER})`);

/** The length of the autolink or the HTML that starts `text`, or 0. */
export function rawLength(text: string): number {
  return RAW.exec(text)?.[0].length ?? 0;
}
