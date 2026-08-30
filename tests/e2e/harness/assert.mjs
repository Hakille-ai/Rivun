export class AssertionError extends Error {
  constructor(message, actual, expected) {
    super(message);
    this.name = 'AssertionError';
    this.actual = actual;
    this.expected = expected;
  }
}

export function assert(condition, message = 'Assertion failed') {
  if (!condition) {
    throw new AssertionError(message, condition, true);
  }
}

export function assertEqual(actual, expected, message = null) {
  if (actual !== expected) {
    const msg = message || 'Expected ' + JSON.stringify(expected) + ', got ' + JSON.stringify(actual);
    throw new AssertionError(msg, actual, expected);
  }
}

export function assertDeepEqual(actual, expected, message = null) {
  const aStr = JSON.stringify(actual);
  const eStr = JSON.stringify(expected);
  if (aStr !== eStr) {
    const msg = message || 'Deep equality mismatch:\nActual: ' + aStr + '\nExpected: ' + eStr;
    throw new AssertionError(msg, actual, expected);
  }
}

export function assertThrows(fn, expectedSubstring = null, message = null) {
  let thrown = false;
  let error = null;
  try {
    fn();
  } catch (err) {
    thrown = true;
    error = err;
  }
  if (!thrown) {
    throw new AssertionError(message || 'Expected function to throw an error, but it did not', null, 'Error');
  }
  if (expectedSubstring && !String(error.message).includes(expectedSubstring)) {
    throw new AssertionError(
      message || ('Expected error message containing: ' + expectedSubstring + ', got: ' + error.message),
      error.message,
      expectedSubstring
    );
  }
}

export async function assertRejects(asyncFn, expectedSubstring = null, message = null) {
  let thrown = false;
  let error = null;
  try {
    await asyncFn();
  } catch (err) {
    thrown = true;
    error = err;
  }
  if (!thrown) {
    throw new AssertionError(message || 'Expected async function to reject, but it resolved', null, 'Error');
  }
  if (expectedSubstring && !String(error.message).includes(expectedSubstring)) {
    throw new AssertionError(
      message || ('Expected rejection containing: ' + expectedSubstring + ', got: ' + error.message),
      error.message,
      expectedSubstring
    );
  }
}

export function assertMatches(str, regex, message = null) {
  if (!regex.test(str)) {
    throw new AssertionError(message || ('String ' + JSON.stringify(str) + ' did not match regex ' + regex), str, regex);
  }
}
