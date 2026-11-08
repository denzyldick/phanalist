<?php
// Bad: cookie without HttpOnly flag
setcookie("session", $value);
