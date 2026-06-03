// api_server_new/load_tests/k6_script.js
//
// Run: k6 run k6_script.js
// Requires: k6 installed — https://k6.io/docs/getting-started/installation/

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// ── Custom metrics ────────────────────────────────────────────────────────────
const buyErrorRate  = new Rate('buy_errors');
const buyDuration   = new Trend('buy_duration_ms', true);

// ── Stages: ramp up → sustain → ramp down ────────────────────────────────────
export const options = {
  stages: [
    { duration: '15s', target: 10  }, // ramp up
    { duration: '30s', target: 50  }, // sustain load
    { duration: '15s', target: 100 }, // stress spike
    { duration: '15s', target: 0   }, // ramp down
  ],
  thresholds: {
    http_req_duration:    ['p(95)<500', 'p(99)<1000'], // 95th < 500ms, 99th < 1s
    http_req_failed:      ['rate<0.01'],                // <1% overall failures
    buy_errors:           ['rate<0.02'],                // <2% buy failures
    buy_duration_ms:      ['p(99)<800'],
  },
};

const BASE = __ENV.BASE_URL || 'http://localhost:8090';

// ── Helpers ───────────────────────────────────────────────────────────────────
function randomEmail() {
  return `user_${__VU}_${Date.now()}@test.com`;
}

function jsonPost(path, body, jar) {
  return http.post(`${BASE}${path}`, JSON.stringify(body), {
    headers: { 'Content-Type': 'application/json' },
    jar,
  });
}

// ── Main scenario ─────────────────────────────────────────────────────────────
export default function () {
  const jar   = http.cookieJar();
  const email = randomEmail();

  // 1. Register
  const regRes = jsonPost('/api/register', {
    name:     'Load Test User',
    email,
    password: 'loadtest123',
  }, jar);

  check(regRes, { 'register: 201': (r) => r.status === 201 });

  // 2. Login
  const loginRes = jsonPost('/api/login', { email, password: 'loadtest123' }, jar);
  check(loginRes, { 'login: 200': (r) => r.status === 200 });

  sleep(0.3);

  // 3. List assets (GET — no auth required)
  const assetsRes = http.get(`${BASE}/api/assets`);
  check(assetsRes, {
    'assets: 200':          (r) => r.status === 200,
    'assets: has Bitcoin':  (r) => r.json('assets.0.asset_type') === 'Bitcoin',
  });

  sleep(0.2);

  // 4. View wallet (auth required via cookie jar)
  const walletRes = http.get(`${BASE}/api/wallet`, { jar });
  check(walletRes, { 'wallet: 200': (r) => r.status === 200 });

  sleep(0.2);

  // 5. Buy order — USD (cheapest asset, avoids balance errors in most runs)
  const buyStart = Date.now();
  const buyRes   = jsonPost('/api/wallet/buy', { symbol: 'USD', quantity: 1 }, jar);
  buyDuration.add(Date.now() - buyStart);

  const buyOk = check(buyRes, { 'buy: 200': (r) => r.status === 200 });
  buyErrorRate.add(!buyOk);

  sleep(0.5);
}
