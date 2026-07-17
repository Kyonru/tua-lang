"use strict";

function formatError(error) {
  if (!error) {
    return "";
  }

  return typeof error.message === "string" && error.message ? error.message : String(error);
}

module.exports = {
  formatError,
};
