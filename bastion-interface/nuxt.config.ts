// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  modules: ['@nuxt/eslint', '@nuxt/ui'],

  devtools: {
    enabled: true
  },

  css: ['~/assets/css/main.css'],

  colorMode: {
    preference: 'dark',
    fallback: 'dark'
  },

  // Override via NUXT_BASTION_BACKEND_URL / NUXT_BASTION_API_TOKEN env vars.
  runtimeConfig: {
    bastionBackendUrl: 'http://localhost:8080',
    bastionApiToken: '',
    public: {}
  },

  compatibilityDate: '2025-01-15',

  nitro: {
    storage: {
      latex: {
        driver: 'fs',
        base: '.data/latex'
      }
    }
  },

  eslint: {
    config: {
      stylistic: {
        commaDangle: 'never',
        braceStyle: '1tbs'
      }
    }
  }
})
