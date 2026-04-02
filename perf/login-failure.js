import http from 'k6/http';
import { check } from 'k6';

export const options = {
  vus: 20,
  duration: '30s',
  summaryTrendStats: ['avg', 'med', 'p(50)', 'p(95)', 'p(99)', 'min', 'max'],
};

const url = __ENV.LOGIN_BASE_URL || 'http://127.0.0.1:3000/login';

const params = {
  headers: {
    'Content-Type': 'application/json',
  },
};

export default function () {
  // Use unknown usernames so this scenario stays read-only and does not lock seeded accounts.
  const userIndex = ((__VU - 1) * 1000000 + __ITER) % 100 + 1;
  const username = `missing-${userIndex}`;
  const payload = JSON.stringify({
    username,
    password: 'wrong-password',
  });
  const response = http.post(url, payload, params);

  check(response, {
    'status is 401': (res) => res.status === 401,
    'body status is error': (res) => res.json('status') === 'error',
  });
}
