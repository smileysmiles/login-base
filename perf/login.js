import http from 'k6/http';
import { check } from 'k6';

export const options = {
  vus: 20,
  duration: '30s',
  summaryTrendStats: ['avg', 'med', 'p(50)', 'p(95)', 'p(99)', 'min', 'max'],
};

const url = __ENV.LOGIN_BASE_URL || 'http://127.0.0.1:3000/login';

const payload = JSON.stringify({
  username: 'demo',
  password: 'password',
});

const params = {
  headers: {
    'Content-Type': 'application/json',
  },
};

export default function () {
  const response = http.post(url, payload, params);

  check(response, {
    'status is 200': (res) => res.status === 200,
    'body status is ok': (res) => res.json('status') === 'ok',
  });
}
