<?php
// Bad: XXE via user-controlled XML
$xml = $_GET['xml'] ?? '';
simplexml_load_string($xml);
