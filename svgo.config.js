export default {
  multipass: true,
  plugins: [
    'preset-default',
    // Convert deprecated xlink:href to modern href
    'removeXlink',
  ],
};
