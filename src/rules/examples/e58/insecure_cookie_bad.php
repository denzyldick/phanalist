<?php

setcookie('session', $value);
setcookie('session', $value, ['secure' => true]);
setrawcookie('session', $value, 0, '/', '', true, false);
session_set_cookie_params(['httponly' => true]);
