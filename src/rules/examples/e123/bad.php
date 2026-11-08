<?php
// Bad: XSS via echo
echo $_GET['name'];
