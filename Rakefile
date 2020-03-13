require 'securerandom'
require 'shellwords'

TARGET = ENV['TARGET'] || 'arm-unknown-linux-gnueabihf'

RPI = ENV['RPI'] || 'cistern.local'
HOST = "pi@#{RPI}"

desc 'compile binary'
task :build do
  sh 'cross', 'build', '--release', '--features', 'server', '--target', TARGET, '-v'
end

desc 'set time zone on Raspberry Pi'
task :setup_timezone do
  sh 'ssh', HOST, 'sudo', 'timedatectl', 'set-timezone', 'Europe/Vienna'
end

desc 'set hostname on Raspberry Pi'
task :setup_hostname do
  sh 'ssh', HOST, <<~SH
    if ! dpkg -s dnsutils >/dev/null; then
      sudo apt-get update
      sudo apt-get install -y dnsutils
    fi

    hostname="$(dig -4 +short -x "$(hostname -I | awk '{print $1}')")"
    hostname="${hostname%%.local.}"

    if [ -n "${hostname}" ]; then
      echo "${hostname}" | sudo tee /etc/hostname >/dev/null
    fi
  SH
end

desc 'set up I2C on Raspberry Pi'
task :setup_i2c do
  sh 'ssh', HOST, 'sudo', 'raspi-config', 'nonint', 'do_i2c', '0'

  r, w = IO.pipe

  w.puts <<~CFG
    SUBSYSTEM=="i2c-dev", ATTR{name}=="bcm2835 I2C adapter", SYMLINK+="i2c", TAG+="systemd"
  CFG
  w.close

  sh 'ssh', HOST, 'sudo', 'tee', '/lib/udev/rules.d/99-i2c.rules', in: r
end

desc 'set up watchdog on Raspberry Pi'
task :setup_watchdog do
  sh 'ssh', HOST, <<~SH
    if ! dpkg -s watchdog >/dev/null; then
      sudo apt-get update
      sudo apt-get install -y watchdog
    fi
  SH

  r, w = IO.pipe

  w.puts 'bcm2835_wdt'
  w.close

  sh 'ssh', HOST, 'sudo', 'tee', '/etc/modules-load.d/bcm2835_wdt.conf', in: r

  gateway_ip = %x(#{['ssh', HOST, 'ip', 'route'].shelljoin})[/via (\d+.\d+.\d+.\d+) /, 1]

  raise if gateway_ip.empty?

  r, w = IO.pipe

  w.puts <<~CFG
    watchdog-device	= /dev/watchdog
    ping = #{gateway_ip}
  CFG
  w.close

  sh 'ssh', HOST, 'sudo', 'tee', '/etc/watchdog.conf', in: r
  sh 'ssh', HOST, 'sudo', 'systemctl', 'enable', 'watchdog'
end

task :setup => [:setup_timezone, :setup_hostname, :setup_i2c, :setup_watchdog]

desc 'deploy binary and service configuration to Raspberry Pi'
task :deploy => :build do
  sh 'rsync', '--rsync-path', 'sudo rsync', "target/#{TARGET}/release/cistern", "#{HOST}:/usr/local/bin/cistern"

  r, w = IO.pipe

  w.write <<~CFG
    [Unit]
    Description=cistern
    BindsTo=dev-i2c.device
    After=dev-i2c.device
    # StartLimitAction=reboot
    StartLimitIntervalSec=60
    StartLimitBurst=10

    [Service]
    Type=simple
    Environment=I2C_DEVICE=/dev/i2c
    Environment=RUST_LOG=info
    Environment=ROCKET_PORT=80
    Environment=ROCKET_SECRET_KEY="#{SecureRandom.base64(32)}"
    ExecStart=/usr/local/bin/cistern
    Restart=always
    RestartSec=1

    [Install]
    WantedBy=multi-user.target
  CFG
  w.close

  sh 'ssh', HOST, 'sudo', 'tee', '/etc/systemd/system/cistern.service', in: r
  sh 'ssh', HOST, 'sudo', 'systemctl', 'enable', 'cistern'
  sh 'ssh', HOST, 'sudo', 'systemctl', 'restart', 'cistern'
end

desc 'show service log'
task :log do
  sh 'ssh', HOST, '-t', 'journalctl', '-f', '-u', 'cistern'
end

task :default => :build
