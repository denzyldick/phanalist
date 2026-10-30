<?php

setcookie('session', $value, [
    'secure' => true,
    'httponly' => true,
    'samesite' => 'Lax',
]);
setcookie('session', $value, ['secure' => true, 'httpOnly' => true]);
session_set_cookie_params(['secure' => true, 'HttpOnly' => true]);
setcookie('session', $value, 0, '/', '', true, true);
session_set_cookie_params(['secure' => true, 'httponly' => true]);
session_set_cookie_params(0, '/', '', true, true);
setcookie('preferences', $value, $options);
session_set_cookie_params($options);
setcookie(name: 'session', value: $value, expires_or_options: ['secure' => true, 'httponly' => true]);
session_set_cookie_params(lifetime_or_options: ['secure' => true, 'httponly' => true]);
session_set_cookie_params(0, path: '/', secure: true, httponly: true);
