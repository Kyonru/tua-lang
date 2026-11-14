-- A locale may select a plural form absent from the catalog. The required
-- `other` text remains a usable translation in that case.
package.path = (arg[1] or "vendor/runtime") .. "/?/init.lua;" .. package.path
package.preload.tuai18n_config = function()
  return {
    default_locale = "en",
    fallback_locale = "en",
    catalogs = {
      { locale = "en", module = "test_i18n_en" },
      { locale = "ru", module = "test_i18n_ru" },
    },
  }
end
package.preload.test_i18n_en = function()
  return { en = { coins = { one = "one", other = "other" } } }
end
package.preload.test_i18n_ru = function()
  return { ru = { coins = { one = "one", other = "fallback" } } }
end

local I18n = require("tuai18n")
assert(I18n.t("coins", { count = 2 }, "ru") == "fallback")
assert(I18n.getLocale() == "en")
