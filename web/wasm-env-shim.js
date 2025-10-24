// workaround to provide env.now() for wasm module in browser environment
export const now = () => {
  return Date.now() * 1000; // Return microseconds
};

export default { now };