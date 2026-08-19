const { resolveConsumerManifest } = require('./shell-consumer');
const { buildManifest, writeBuildResolution } = require('./shell-profile');

module.exports = {
  buildManifest,
  resolveConsumerManifest,
  writeBuildResolution,
};
